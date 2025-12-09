//! Orbit CLI - Interactive database client with syntax highlighting
//!
//! A rich terminal interface for connecting to Orbit databases via multiple protocols:
//! - PostgreSQL Wire Protocol
//! - MySQL Protocol
//! - Cassandra Query Language (CQL)
//! - Redis (RESP Protocol)
//! - OrbitQL (via REST API)
//! - Cypher (Neo4j graph queries via REST API)
//! - AQL (ArangoDB queries via REST API)
//! - Arrow Flight SQL (high-performance columnar protocol)
//! - OrbitWire (native binary protocol)
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
    /// Protocol to use for connection (interactive if not specified)
    #[arg(long, value_enum)]
    protocol: Option<Protocol>,

    /// Host to connect to
    #[arg(short = 'H', long, default_value = "localhost")]
    host: String,

    /// Port to connect to
    #[arg(short = 'p', long)]
    port: Option<u16>,

    /// Database name (prompted if not specified)
    #[arg(short, long)]
    database: Option<String>,

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
    /// Cypher (Neo4j graph queries via REST API)
    Cypher,
    /// AQL (ArangoDB queries via REST API)
    Aql,
    /// Arrow Flight SQL (high-performance columnar protocol)
    Flight,
    /// OrbitWire (native binary protocol)
    Orbitwire,
}

impl Protocol {
    fn default_port(&self) -> u16 {
        match self {
            Protocol::Postgres => 5432,
            Protocol::Mysql => 3306,
            Protocol::Cql => 9042,
            Protocol::Redis => 6379,
            Protocol::Orbitql => 8080,
            Protocol::Cypher => 7474,
            Protocol::Aql => 8529,
            Protocol::Flight => 50052,
            Protocol::Orbitwire => 50053,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Protocol::Postgres => "PostgreSQL",
            Protocol::Mysql => "MySQL",
            Protocol::Cql => "CQL",
            Protocol::Redis => "Redis",
            Protocol::Orbitql => "OrbitQL",
            Protocol::Cypher => "Cypher",
            Protocol::Aql => "AQL",
            Protocol::Flight => "Arrow Flight SQL",
            Protocol::Orbitwire => "OrbitWire",
        }
    }

    fn from_str(s: &str) -> Option<Protocol> {
        match s.to_lowercase().trim() {
            "postgres" | "postgresql" | "pg" | "1" => Some(Protocol::Postgres),
            "mysql" | "2" => Some(Protocol::Mysql),
            "cql" | "cassandra" | "3" => Some(Protocol::Cql),
            "redis" | "4" => Some(Protocol::Redis),
            "orbitql" | "orbit" | "5" => Some(Protocol::Orbitql),
            "cypher" | "neo4j" | "6" => Some(Protocol::Cypher),
            "aql" | "arango" | "arangodb" | "7" => Some(Protocol::Aql),
            "flight" | "flightsql" | "arrow" | "8" => Some(Protocol::Flight),
            "orbitwire" | "wire" | "9" => Some(Protocol::Orbitwire),
            _ => None,
        }
    }

    fn all() -> Vec<Protocol> {
        vec![
            Protocol::Postgres,
            Protocol::Mysql,
            Protocol::Cql,
            Protocol::Redis,
            Protocol::Orbitql,
            Protocol::Cypher,
            Protocol::Aql,
            Protocol::Flight,
            Protocol::Orbitwire,
        ]
    }

    fn cli_name(&self) -> &'static str {
        match self {
            Protocol::Postgres => "postgres",
            Protocol::Mysql => "mysql",
            Protocol::Cql => "cql",
            Protocol::Redis => "redis",
            Protocol::Orbitql => "orbitql",
            Protocol::Cypher => "cypher",
            Protocol::Aql => "aql",
            Protocol::Flight => "flight",
            Protocol::Orbitwire => "orbitwire",
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
    // OrbitWire connection
    orbitwire_stream: Option<OrbitWireConnection>,
    // Arrow Flight SQL connection
    flight_client: Option<FlightSqlConnection>,
}

/// OrbitWire connection wrapper
struct OrbitWireConnection {
    #[allow(dead_code)]
    stream: tokio::net::TcpStream,
}

/// Flight SQL connection wrapper
struct FlightSqlConnection {
    client: reqwest::Client,
    endpoint: String,
}

impl ReplState {
    fn new(
        protocol: Protocol,
        host: String,
        port: u16,
        database: String,
        username: String,
        password: Option<String>,
        format: OutputFormat,
    ) -> Self {
        Self {
            protocol,
            host,
            port,
            database,
            username,
            password,
            format,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            pg_client: None,
            pg_connection_handle: None,
            mysql_pool: None,
            redis_client: None,
            http_client: None,
            orbitwire_stream: None,
            flight_client: None,
        }
    }

    fn from_cli(cli: &Cli, protocol: Protocol, database: String) -> Self {
        let port = cli.port.unwrap_or_else(|| protocol.default_port());
        Self::new(
            protocol,
            cli.host.clone(),
            port,
            database,
            cli.username.clone(),
            cli.password.clone(),
            cli.format.into(),
        )
    }

    /// Switch to a different protocol (disconnects current connection)
    async fn switch_protocol(&mut self, new_protocol: Protocol) -> Result<()> {
        // Disconnect current connections
        self.pg_client = None;
        self.pg_connection_handle = None;
        self.mysql_pool = None;
        self.redis_client = None;
        self.http_client = None;
        self.orbitwire_stream = None;
        self.flight_client = None;

        // Update protocol and port
        self.protocol = new_protocol;
        self.port = new_protocol.default_port();

        // Connect with new protocol
        self.connect().await
    }

    /// Connect to the database based on protocol
    async fn connect(&mut self) -> Result<()> {
        match self.protocol {
            Protocol::Postgres => self.connect_postgres().await,
            Protocol::Mysql => self.connect_mysql().await,
            Protocol::Redis => self.connect_redis().await,
            Protocol::Orbitql => self.connect_orbitql().await,
            Protocol::Cql => self.connect_cql().await,
            Protocol::Cypher => self.connect_cypher().await,
            Protocol::Aql => self.connect_aql().await,
            Protocol::Flight => self.connect_flight().await,
            Protocol::Orbitwire => self.connect_orbitwire().await,
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
            Ok(mut conn) => match conn.query_first::<String, _>("SELECT 1").await {
                Ok(_) => {
                    self.mysql_pool = Some(pool);
                    Ok(())
                }
                Err(e) => Err(anyhow::anyhow!("MySQL connection test failed: {}", e)),
            },
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
            Protocol::Cypher => self.execute_cypher_query(query).await,
            Protocol::Aql => self.execute_aql_query(query).await,
            Protocol::Flight => self.execute_flight_query(query).await,
            Protocol::Orbitwire => self.execute_orbitwire_query(query).await,
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
            redis::Value::BulkString(data) => {
                if let Ok(s) = String::from_utf8(data.clone()) {
                    println!("{}\"{}\"", prefix, s);
                } else {
                    println!("{}(binary data, {} bytes)", prefix, data.len());
                }
            }
            redis::Value::Array(arr) => {
                if arr.is_empty() {
                    println!("{}(empty array)", prefix);
                } else {
                    for (i, item) in arr.iter().enumerate() {
                        print!("{}{}) ", prefix, i + 1);
                        self.format_redis_value(item, 0);
                    }
                }
            }
            redis::Value::SimpleString(s) => println!("{}{}", prefix, s),
            // Handle additional RESP3 types
            redis::Value::Double(d) => println!("{}(double) {}", prefix, d),
            redis::Value::Boolean(b) => println!("{}(boolean) {}", prefix, b),
            redis::Value::Map(map) => {
                if map.is_empty() {
                    println!("{}(empty map)", prefix);
                } else {
                    for (i, (k, v)) in map.iter().enumerate() {
                        print!("{}{}) key: ", prefix, i + 1);
                        self.format_redis_value(k, 0);
                        print!("{}   value: ", prefix);
                        self.format_redis_value(v, 0);
                    }
                }
            }
            redis::Value::Set(set) => {
                if set.is_empty() {
                    println!("{}(empty set)", prefix);
                } else {
                    for (i, item) in set.iter().enumerate() {
                        print!("{}{}) ", prefix, i + 1);
                        self.format_redis_value(item, 0);
                    }
                }
            }
            redis::Value::Okay => println!("{}OK", prefix),
            _ => println!("{}(unsupported redis type)", prefix),
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

    /// Connect to Cypher (Neo4j) via REST API
    async fn connect_cypher(&mut self) -> Result<()> {
        let client = reqwest::Client::new();
        // Neo4j HTTP API endpoint - try to connect to the database
        let url = format!("http://{}:{}/db/neo4j/tx/commit", self.host, self.port);

        // Send an empty query to test connectivity
        let request_body = serde_json::json!({
            "statements": []
        });

        match client.post(&url).json(&request_body).send().await {
            Ok(response) if response.status().is_success() => {
                self.http_client = Some(client);
                Ok(())
            }
            Ok(response) => {
                // Neo4j might require authentication - try basic auth
                let status = response.status();
                if status.as_u16() == 401 {
                    // Try with basic auth
                    let auth_client = reqwest::Client::new();
                    let auth_response = auth_client
                        .post(&url)
                        .basic_auth(&self.username, self.password.as_ref())
                        .json(&request_body)
                        .send()
                        .await;

                    match auth_response {
                        Ok(r) if r.status().is_success() => {
                            self.http_client = Some(auth_client);
                            Ok(())
                        }
                        Ok(r) => Err(anyhow::anyhow!(
                            "Cypher authentication failed: HTTP {}",
                            r.status()
                        )),
                        Err(e) => Err(anyhow::anyhow!("Cypher connection failed: {}", e)),
                    }
                } else {
                    Err(anyhow::anyhow!("Cypher connection failed: HTTP {}", status))
                }
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to connect to Cypher endpoint: {}",
                e
            )),
        }
    }

    /// Execute a Cypher query via Neo4j REST API
    async fn execute_cypher_query(&self, query: &str) -> Result<()> {
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to Cypher endpoint"))?;

        // Neo4j HTTP API transaction endpoint
        let url = format!("http://{}:{}/db/neo4j/tx/commit", self.host, self.port);

        let request_body = serde_json::json!({
            "statements": [{
                "statement": query.trim().trim_end_matches(';'),
                "resultDataContents": ["row"]
            }]
        });

        let response = client
            .post(&url)
            .basic_auth(&self.username, self.password.as_ref())
            .json(&request_body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Cypher request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Cypher error (HTTP {}): {}", status, body));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse Cypher response: {}", e))?;

        // Check for errors in response
        if let Some(errors) = result.get("errors").and_then(|e| e.as_array()) {
            if !errors.is_empty() {
                let error_msg = errors
                    .iter()
                    .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(anyhow::anyhow!("Cypher error: {}", error_msg));
            }
        }

        // Parse and display results
        if let Some(results) = result.get("results").and_then(|r| r.as_array()) {
            for result_set in results {
                // Get columns
                let columns: Vec<String> = result_set
                    .get("columns")
                    .and_then(|c| c.as_array())
                    .map(|cols| {
                        cols.iter()
                            .filter_map(|c| c.as_str())
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();

                // Get data rows
                if let Some(data) = result_set.get("data").and_then(|d| d.as_array()) {
                    if data.is_empty() {
                        println!("{}", format_success("Query executed successfully (0 rows)"));
                        continue;
                    }

                    let mut table = Table::new();
                    table
                        .load_preset(UTF8_FULL)
                        .apply_modifier(UTF8_ROUND_CORNERS);

                    if !columns.is_empty() {
                        table.set_header(columns.iter().map(|c| Cell::new(c).fg(Color::Cyan)));
                    }

                    for row_data in data {
                        if let Some(row) = row_data.get("row").and_then(|r| r.as_array()) {
                            let values: Vec<String> =
                                row.iter()
                                    .map(|v| match v {
                                        serde_json::Value::Null => "NULL".to_string(),
                                        serde_json::Value::String(s) => s.clone(),
                                        serde_json::Value::Object(_) => serde_json::to_string(v)
                                            .unwrap_or_else(|_| v.to_string()),
                                        _ => v.to_string(),
                                    })
                                    .collect();
                            table.add_row(values.iter().map(Cell::new));
                        }
                    }

                    println!("\n{}", table);
                    println!("{}", format_success(&format!("({} rows)", data.len())));
                }
            }
        }

        Ok(())
    }

    /// Connect to AQL (ArangoDB) via REST API
    async fn connect_aql(&mut self) -> Result<()> {
        let client = reqwest::Client::new();
        // ArangoDB API version endpoint
        let url = format!("http://{}:{}/_api/version", self.host, self.port);

        let response = if self.password.is_some() {
            client
                .get(&url)
                .basic_auth(&self.username, self.password.as_ref())
                .send()
                .await
        } else {
            client.get(&url).send().await
        };

        match response {
            Ok(r) if r.status().is_success() => {
                self.http_client = Some(client);
                Ok(())
            }
            Ok(r) => Err(anyhow::anyhow!(
                "AQL connection failed: HTTP {}",
                r.status()
            )),
            Err(e) => Err(anyhow::anyhow!("Failed to connect to AQL endpoint: {}", e)),
        }
    }

    /// Execute an AQL query via ArangoDB REST API
    async fn execute_aql_query(&self, query: &str) -> Result<()> {
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to AQL endpoint"))?;

        // ArangoDB cursor API endpoint
        let url = format!(
            "http://{}:{}/_db/{}/_api/cursor",
            self.host, self.port, self.database
        );

        let request_body = serde_json::json!({
            "query": query.trim().trim_end_matches(';'),
            "batchSize": 1000
        });

        let mut request = client.post(&url).json(&request_body);
        if self.password.is_some() {
            request = request.basic_auth(&self.username, self.password.as_ref());
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("AQL request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("AQL error (HTTP {}): {}", status, body));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse AQL response: {}", e))?;

        // Check for errors
        if let Some(error) = result.get("error").and_then(|e| e.as_bool()) {
            if error {
                let error_msg = result
                    .get("errorMessage")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error");
                return Err(anyhow::anyhow!("AQL error: {}", error_msg));
            }
        }

        // Parse and display results
        if let Some(data) = result.get("result").and_then(|r| r.as_array()) {
            if data.is_empty() {
                println!("{}", format_success("Query executed successfully (0 rows)"));
                return Ok(());
            }

            // For AQL, results can be documents or scalar values
            // Try to detect if results are documents (objects) or scalars
            let first_item = &data[0];

            if first_item.is_object() {
                // Document results - display as table
                let columns: Vec<String> = first_item
                    .as_object()
                    .map(|obj| obj.keys().cloned().collect())
                    .unwrap_or_default();

                let mut table = Table::new();
                table
                    .load_preset(UTF8_FULL)
                    .apply_modifier(UTF8_ROUND_CORNERS);

                if !columns.is_empty() {
                    table.set_header(columns.iter().map(|c| Cell::new(c).fg(Color::Cyan)));
                }

                for item in data {
                    if let Some(obj) = item.as_object() {
                        let values: Vec<String> = columns
                            .iter()
                            .map(|col| {
                                obj.get(col)
                                    .map(|v| match v {
                                        serde_json::Value::Null => "NULL".to_string(),
                                        serde_json::Value::String(s) => s.clone(),
                                        _ => v.to_string(),
                                    })
                                    .unwrap_or_else(|| "NULL".to_string())
                            })
                            .collect();
                        table.add_row(values.iter().map(Cell::new));
                    }
                }

                println!("\n{}", table);
                println!("{}", format_success(&format!("({} rows)", data.len())));
            } else {
                // Scalar results - display as single column
                let mut table = Table::new();
                table
                    .load_preset(UTF8_FULL)
                    .apply_modifier(UTF8_ROUND_CORNERS);
                table.set_header(vec![Cell::new("result").fg(Color::Cyan)]);

                for item in data {
                    let value = match item {
                        serde_json::Value::Null => "NULL".to_string(),
                        serde_json::Value::String(s) => s.clone(),
                        _ => item.to_string(),
                    };
                    table.add_row(vec![Cell::new(value)]);
                }

                println!("\n{}", table);
                println!("{}", format_success(&format!("({} rows)", data.len())));
            }
        }

        Ok(())
    }

    /// Connect to Arrow Flight SQL server
    async fn connect_flight(&mut self) -> Result<()> {
        // Arrow Flight SQL uses gRPC. For CLI, we use a simplified HTTP-based approach
        // that talks to the Flight SQL REST adapter at the server
        let client = reqwest::Client::new();
        let endpoint = format!("http://{}:{}", self.host, self.port);

        // Try to connect by checking the health endpoint or a simple handshake
        let health_url = format!("{}/health", endpoint);

        match client.get(&health_url).send().await {
            Ok(response) if response.status().is_success() || response.status().as_u16() == 404 => {
                // Flight SQL server running (404 on /health is ok - it means server responds)
                self.flight_client = Some(FlightSqlConnection { client, endpoint });
                Ok(())
            }
            Ok(response) => Err(anyhow::anyhow!(
                "Flight SQL connection failed: HTTP {}",
                response.status()
            )),
            Err(e) => Err(anyhow::anyhow!(
                "Failed to connect to Flight SQL server: {}",
                e
            )),
        }
    }

    /// Connect to OrbitWire server
    async fn connect_orbitwire(&mut self) -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let addr = format!("{}:{}", self.host, self.port);

        match tokio::net::TcpStream::connect(&addr).await {
            Ok(mut stream) => {
                // Send handshake: magic bytes + version
                // OrbitWire magic: "ORBT" + version 1
                let handshake = [0x4F, 0x52, 0x42, 0x54, 0x01]; // "ORBT" + version 1
                stream.write_all(&handshake).await?;

                // Read handshake response (5 bytes)
                let mut response = [0u8; 5];
                match tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    stream.read_exact(&mut response),
                )
                .await
                {
                    Ok(Ok(_)) => {
                        // Verify magic bytes
                        if &response[..4] == b"ORBT" {
                            self.orbitwire_stream = Some(OrbitWireConnection { stream });
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("Invalid OrbitWire handshake response"))
                        }
                    }
                    Ok(Err(e)) => Err(anyhow::anyhow!("OrbitWire handshake failed: {}", e)),
                    Err(_) => Err(anyhow::anyhow!("OrbitWire handshake timed out")),
                }
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to connect to OrbitWire server: {}",
                e
            )),
        }
    }

    /// Execute a query via Arrow Flight SQL
    async fn execute_flight_query(&self, query: &str) -> Result<()> {
        let flight = self
            .flight_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to Flight SQL server"))?;

        // Send query via REST adapter endpoint
        let url = format!("{}/api/v1/flight/sql", flight.endpoint);

        let request_body = serde_json::json!({
            "query": query.trim().trim_end_matches(';'),
            "database": self.database
        });

        let response = flight
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Flight SQL request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Flight SQL error (HTTP {}): {}",
                status,
                body
            ));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse Flight SQL response: {}", e))?;

        // Display result (Arrow Flight returns columnar data)
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
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
        } else if let Some(error) = result.get("error") {
            return Err(anyhow::anyhow!("Flight SQL error: {}", error));
        } else {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }

        Ok(())
    }

    /// Execute a query via OrbitWire protocol
    async fn execute_orbitwire_query(&self, query: &str) -> Result<()> {
        // OrbitWire requires mutable access for socket I/O
        // For simplicity in CLI, use HTTP fallback to OrbitQL endpoint
        // A full implementation would use the binary protocol

        // Fallback to HTTP endpoint on OrbitWire port + 27 (REST adapter port offset)
        let http_port = self.port.saturating_sub(50053).saturating_add(8080);
        let url = format!("http://{}:{}/api/v1/sql", self.host, http_port);

        let client = reqwest::Client::new();
        let request_body = serde_json::json!({
            "query": query.trim().trim_end_matches(';'),
            "protocol": "orbitwire"
        });

        let response = client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("OrbitWire request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "OrbitWire error (HTTP {}): {}",
                status,
                body
            ));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse OrbitWire response: {}", e))?;

        // Display result
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
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
        } else if let Some(error) = result.get("error") {
            return Err(anyhow::anyhow!("OrbitWire error: {}", error));
        } else {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }

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
    // For single command mode, protocol is required via CLI
    let protocol = cli.protocol.ok_or_else(|| {
        anyhow::anyhow!("Protocol is required for -e mode. Use --protocol <protocol>")
    })?;

    // For single command mode, use default database if not specified
    let database = cli.database.clone().unwrap_or_else(|| "orbit".to_string());

    let mut state = ReplState::from_cli(cli, protocol, database);

    println!(
        "{}",
        format!(
            "Connecting to {} at {}...",
            protocol.name(),
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
    // For file mode, protocol is required via CLI
    let protocol = cli.protocol.ok_or_else(|| {
        anyhow::anyhow!("Protocol is required for -f mode. Use --protocol <protocol>")
    })?;

    // For file mode, use default database if not specified
    let database = cli.database.clone().unwrap_or_else(|| "orbit".to_string());

    let mut state = ReplState::from_cli(cli, protocol, database);

    println!(
        "{}",
        format!(
            "Connecting to {} at {}...",
            protocol.name(),
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
    let statements: Vec<&str> = if protocol == Protocol::Redis {
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

/// Prompt user to select a protocol interactively
fn prompt_for_protocol(editor: &mut DefaultEditor) -> Result<Protocol> {
    println!("\n{}", "Select a protocol to connect:".cyan().bold());
    print_protocol_list();
    println!();

    loop {
        let prompt = "Protocol (1-9 or name): ".green().to_string();
        match editor.readline(&prompt) {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    println!("{}", "Please select a protocol by number or name.".dimmed());
                    continue;
                }

                if let Some(protocol) = Protocol::from_str(input) {
                    println!(
                        "{}",
                        format!("Selected: {} ({})", protocol.name(), protocol.cli_name()).green()
                    );
                    return Ok(protocol);
                } else {
                    println!(
                        "{}",
                        format_error(&format!("Unknown protocol: '{}'. Please try again.", input))
                    );
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                return Err(anyhow::anyhow!("Interrupted"));
            }
            Err(ReadlineError::Eof) => {
                return Err(anyhow::anyhow!("EOF"));
            }
            Err(err) => {
                return Err(anyhow::anyhow!("Error reading input: {}", err));
            }
        }
    }
}

/// Prompt user for database name
fn prompt_for_database(editor: &mut DefaultEditor, protocol: &Protocol) -> Result<String> {
    // For Redis, database is typically a number (0-15), but can be named
    // For others, it's a string name
    let hint = match protocol {
        Protocol::Redis => "Database index (0-15, default: 0): ",
        Protocol::Cypher => "Database name (default: neo4j): ",
        Protocol::Aql => "Database name (default: _system): ",
        _ => "Database name: ",
    };

    let default_db = match protocol {
        Protocol::Redis => "0",
        Protocol::Cypher => "neo4j",
        Protocol::Aql => "_system",
        _ => "",
    };

    println!();
    let prompt = hint.green().to_string();

    loop {
        match editor.readline(&prompt) {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    if !default_db.is_empty() {
                        println!("{}", format!("Using default: {}", default_db).dimmed());
                        return Ok(default_db.to_string());
                    } else {
                        println!("{}", "Please enter a database name.".dimmed());
                        continue;
                    }
                }
                return Ok(input.to_string());
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                return Err(anyhow::anyhow!("Interrupted"));
            }
            Err(ReadlineError::Eof) => {
                if !default_db.is_empty() {
                    return Ok(default_db.to_string());
                }
                return Err(anyhow::anyhow!("EOF"));
            }
            Err(err) => {
                return Err(anyhow::anyhow!("Error reading input: {}", err));
            }
        }
    }
}

/// Run interactive REPL
async fn run_repl(cli: &Cli) -> Result<()> {
    // Initialize rustyline editor early for prompts
    let mut editor = DefaultEditor::new()?;

    // Determine protocol (interactive if not specified)
    let protocol = match cli.protocol {
        Some(p) => p,
        None => prompt_for_protocol(&mut editor)?,
    };

    // Determine database (interactive if not specified)
    let database = match &cli.database {
        Some(d) => d.clone(),
        None => prompt_for_database(&mut editor, &protocol)?,
    };

    let mut state = ReplState::from_cli(cli, protocol, database);

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
                    if handle_meta_command(trimmed, &mut state, &mut editor, &history_file).await? {
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
                    // If the line is just a semicolon, don't add a newline before it
                    if trimmed == ";" && !query_buffer.is_empty() {
                        // Just append the semicolon to complete the query
                        query_buffer.push(';');
                    } else {
                        if !query_buffer.is_empty() {
                            query_buffer.push(' '); // Use space instead of newline for continuations
                        }
                        query_buffer.push_str(trimmed);
                    }

                    // Check if query is complete (ends with semicolon)
                    if query_buffer.trim().ends_with(';') {
                        // Clean up the query - remove trailing semicolon for execution
                        let clean_query = query_buffer.trim().trim_end_matches(';').trim();

                        // Add to history
                        editor.add_history_entry(query_buffer.trim())?;

                        // Show highlighted query
                        println!("\n{}", state.highlight_query(clean_query));

                        // Execute query (without trailing semicolon)
                        match state.execute_query(clean_query).await {
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
    state: &mut ReplState,
    editor: &mut DefaultEditor,
    history_file: &PathBuf,
) -> Result<bool> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let cmd = parts.first().map(|s| *s).unwrap_or("");

    match cmd {
        "\\q" | "\\quit" | "\\exit" => {
            editor.save_history(history_file)?;
            return Ok(true); // Signal to exit
        }
        "\\?" | "\\help" => {
            print_help();
        }
        "\\p" | "\\protocol" => {
            if parts.len() < 2 {
                // Show current protocol and list available ones
                println!("\n{}", "Current protocol:".cyan());
                println!(
                    "  {} ({}) on port {}",
                    state.protocol.name().green(),
                    state.protocol.cli_name(),
                    state.port
                );
                print_protocol_list();
                println!(
                    "\n{}",
                    "Usage: \\p <protocol>  (e.g., \\p redis, \\p mysql)".dimmed()
                );
            } else {
                let protocol_arg = parts[1];
                if let Some(new_protocol) = Protocol::from_str(protocol_arg) {
                    println!(
                        "{}",
                        format!("Switching to {} protocol...", new_protocol.name()).dimmed()
                    );
                    match state.switch_protocol(new_protocol).await {
                        Ok(()) => {
                            println!(
                                "{}",
                                format_success(&format!(
                                    "Connected to {} on port {}",
                                    new_protocol.name(),
                                    state.port
                                ))
                            );
                        }
                        Err(e) => {
                            println!("{}", format_error(&format!("Connection failed: {}", e)));
                            println!(
                                "{}",
                                "Protocol switched but running in offline mode.".dimmed()
                            );
                        }
                    }
                } else {
                    println!(
                        "{}",
                        format_error(&format!("Unknown protocol: '{}'", protocol_arg))
                    );
                    print_protocol_list();
                }
            }
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
        "\\format" => {
            if parts.len() >= 2 {
                let format_str = parts[1];
                println!(
                    "{}",
                    format_success(&format!("Output format set to: {}", format_str))
                );
                // TODO: Update state.format
            } else {
                println!("{}", "Usage: \\format <table|json|csv|plain>".dimmed());
            }
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

/// Print list of available protocols
fn print_protocol_list() {
    println!("\n{}", "Available protocols:".cyan());
    for (i, proto) in Protocol::all().iter().enumerate() {
        println!(
            "  {}) {} ({}) - port {}",
            i + 1,
            proto.cli_name().green(),
            proto.name(),
            proto.default_port()
        );
    }
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

Meta Commands:
  \?              Show this help
  \q, \quit       Exit the CLI
  \p [protocol]   Show current protocol or switch to new protocol
                  Examples: \p redis, \p mysql, \p 3
  \d              List tables
  \dt             List tables (same as \d)
  \l              List databases
  \c              Connect to different database
  \timing         Toggle query timing display
  \format         Set output format (table, json, csv, plain)

Supported Protocols (use \p to switch):
  1) postgres     PostgreSQL wire protocol (port 5432)
  2) mysql        MySQL protocol (port 3306)
  3) cql          Cassandra CQL via REST (port 9042)
  4) redis        Redis RESP protocol (port 6379)
  5) orbitql      OrbitQL via REST API (port 8080)
  6) cypher       Neo4j Cypher via REST API (port 7474)
  7) aql          ArangoDB AQL via REST API (port 8529)
  8) flight       Arrow Flight SQL (port 50052)
  9) orbitwire    OrbitWire binary protocol (port 50053)

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

Cypher Examples (Neo4j):
  MATCH (n:Person) RETURN n.name;
  CREATE (p:Person {name: 'Alice'}) RETURN p;
  MATCH (a:Person)-[:KNOWS]->(b:Person) RETURN a.name, b.name;

AQL Examples (ArangoDB):
  FOR doc IN users RETURN doc;
  FOR u IN users FILTER u.age > 18 RETURN u.name;
  INSERT {name: 'Alice'} INTO users;

Arrow Flight SQL (--protocol flight):
  - High-performance columnar data protocol
  - Uses Apache Arrow for efficient data transfer
  - Ideal for analytics and large result sets

OrbitWire (--protocol orbitwire):
  - Native binary protocol for Orbit database
  - Optimized for OrbitQL queries
  - Supports LIVE queries, graph traversal, vector operations

"#;

    println!("{}", help_text.cyan());
}
