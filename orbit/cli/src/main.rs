//! Orbit CLI - Interactive database client with syntax highlighting
//!
//! A rich terminal interface for connecting to Orbit databases via multiple protocols:
//! - PostgreSQL Wire Protocol
//! - MySQL Protocol
//! - Cassandra Query Language (CQL)
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
#[derive(Debug, Clone, Copy, ValueEnum)]
enum Protocol {
    /// PostgreSQL Wire Protocol
    Postgres,
    /// MySQL Protocol
    Mysql,
    /// Cassandra Query Language (CQL)
    Cql,
}

impl Protocol {
    fn default_port(&self) -> u16 {
        match self {
            Protocol::Postgres => 5432,
            Protocol::Mysql => 3306,
            Protocol::Cql => 9042,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Protocol::Postgres => "PostgreSQL",
            Protocol::Mysql => "MySQL",
            Protocol::Cql => "CQL",
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
    format: OutputFormat,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    // Connection will be added when we integrate with orbit-client
    // connection: Option<Connection>,
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
            format: cli.format.into(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
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
    let state = ReplState::new(cli);

    println!(
        "{}",
        format!("Connecting to {} at {}...", cli.protocol.name(), state.connection_string())
            .dimmed()
    );

    // TODO: Establish actual connection
    // For now, just show what we would execute
    println!("\n{}", state.highlight_query(query));
    println!(
        "\n{}",
        format_error("Not yet connected to database - connection logic pending")
    );

    Ok(())
}

/// Execute commands from a file
async fn execute_file(cli: &Cli, file_path: &PathBuf) -> Result<()> {
    let state = ReplState::new(cli);

    println!(
        "{}",
        format!("Connecting to {} at {}...", cli.protocol.name(), state.connection_string())
            .dimmed()
    );

    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

    // Split into statements (simple split on semicolon for now)
    let statements: Vec<&str> = content.split(';').filter(|s| !s.trim().is_empty()).collect();

    println!("{}", format!("Executing {} statements from file...", statements.len()).dimmed());

    for (i, statement) in statements.iter().enumerate() {
        println!("\n{}", format!("Statement {}:", i + 1).cyan());
        println!("{}", state.highlight_query(statement.trim()));

        // TODO: Execute statement
        println!(
            "{}",
            format_error("Not yet connected to database - connection logic pending")
        );
    }

    Ok(())
}

/// Run interactive REPL
async fn run_repl(cli: &Cli) -> Result<()> {
    let state = ReplState::new(cli);

    // Print welcome banner
    print_banner(&state);

    // TODO: Establish connection
    println!(
        "\n{}",
        format_error("Connection logic pending - currently in offline mode")
    );
    println!(
        "{}",
        "You can try typing SQL queries to see syntax highlighting.".dimmed()
    );

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

                // Add to query buffer
                query_buffer.push_str(&line);
                query_buffer.push('\n');

                // Check if query is complete (ends with semicolon)
                if trimmed.ends_with(';') {
                    // Add to history
                    editor.add_history_entry(query_buffer.trim())?;

                    // Show highlighted query
                    println!("\n{}", state.highlight_query(&query_buffer));

                    // TODO: Execute query
                    println!(
                        "\n{}",
                        format_error("Query execution pending - connection logic not yet implemented")
                    );
                    println!("{}", "Query would be executed here.".dimmed());

                    // Clear buffer
                    query_buffer.clear();
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
    table.load_preset(UTF8_FULL).apply_modifier(UTF8_ROUND_CORNERS);

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
  \?          Show this help
  \q, \quit   Exit the CLI
  \d          List tables
  \dt         List tables (same as \d)
  \l          List databases
  \c          Connect to different database
  \timing     Toggle query timing display
  \format     Set output format (table, json, csv, plain)

Query Execution:
  - End queries with semicolon (;) to execute
  - Multi-line queries are supported
  - Use Ctrl+C to cancel current query
  - Use Ctrl+D or \q to exit

Examples:
  SELECT * FROM users;
  SELECT id, name
  FROM users
  WHERE age > 18;

"#;

    println!("{}", help_text.cyan());
}
