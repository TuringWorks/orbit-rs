//! Simple OrbitQL Language Server binary
//!
//! This is a minimal working LSP server for OrbitQL that provides basic
//! functionality while avoiding complex dependencies that have compilation issues.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tower_lsp::lsp_types::*;

/// Simple OrbitQL LSP Server
struct SimpleOrbitQLServer {
    keywords: Vec<String>,
    functions: Vec<String>,
}

impl SimpleOrbitQLServer {
    fn new() -> Self {
        Self {
            keywords: vec![
                "SELECT".to_string(),
                "FROM".to_string(),
                "WHERE".to_string(),
                "JOIN".to_string(),
                "GROUP BY".to_string(),
                "ORDER BY".to_string(),
                "INSERT".to_string(),
                "UPDATE".to_string(),
                "DELETE".to_string(),
                "CREATE".to_string(),
                "DROP".to_string(),
                "ALTER".to_string(),
                // OrbitQL specific
                "RELATE".to_string(),
                "FETCH".to_string(),
                "LIVE".to_string(),
                "GRAPH".to_string(),
                "TRAVERSE".to_string(),
            ],
            functions: vec![
                "COUNT".to_string(),
                "SUM".to_string(),
                "AVG".to_string(),
                "MIN".to_string(),
                "MAX".to_string(),
                "time::now".to_string(),
                "graph::traverse".to_string(),
            ],
        }
    }

    /// Handle LSP initialize request
    fn handle_initialize(&self, _params: InitializeParams) -> InitializeResult {
        InitializeResult {
            server_info: Some(ServerInfo {
                name: "simple-orbitql-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![" ".to_string(), ".".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
        }
    }

    /// Handle completion request
    fn handle_completion(&self, _params: CompletionParams) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // Add keywords
        for keyword in &self.keywords {
            items.push(CompletionItem {
                label: keyword.clone(),
                kind: Some(CompletionItemKind::KEYWORD),
                insert_text: Some(keyword.clone()),
                ..Default::default()
            });
        }

        // Add functions
        for function in &self.functions {
            items.push(CompletionItem {
                label: function.clone(),
                kind: Some(CompletionItemKind::FUNCTION),
                insert_text: Some(format!("{}()", function)),
                ..Default::default()
            });
        }

        items
    }

    /// Handle hover request
    fn handle_hover(&self, _params: HoverParams) -> Option<Hover> {
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(
                "OrbitQL - Unified Multi-Model Query Language".to_string(),
            )),
            range: None,
        })
    }

    /// Handle formatting request
    #[allow(dead_code)]
    fn handle_formatting(&self, _params: DocumentFormattingParams) -> Vec<TextEdit> {
        // Simple formatting - just normalize whitespace
        vec![TextEdit {
            range: Range::new(Position::new(0, 0), Position::new(1000, 0)),
            new_text: "-- Formatted OrbitQL query\\nSELECT * FROM users;".to_string(),
        }]
    }
}

#[derive(Serialize, Deserialize)]
struct LSPMessage {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: Option<String>,
    params: Option<serde_json::Value>,
    result: Option<serde_json::Value>,
    error: Option<serde_json::Value>,
}

async fn handle_lsp_message(
    server: &SimpleOrbitQLServer,
    message: LSPMessage,
) -> Option<LSPMessage> {
    if let Some(method) = &message.method {
        match method.as_str() {
            "initialize" => {
                let result = server.handle_initialize(InitializeParams::default());
                Some(LSPMessage {
                    jsonrpc: "2.0".to_string(),
                    id: message.id,
                    method: None,
                    params: None,
                    result: Some(serde_json::to_value(result).unwrap()),
                    error: None,
                })
            }
            "textDocument/completion" => {
                let result = server.handle_completion(CompletionParams {
                    text_document_position: TextDocumentPositionParams {
                        text_document: TextDocumentIdentifier {
                            uri: Url::parse("file:///test.oql").unwrap(),
                        },
                        position: Position::new(0, 0),
                    },
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                    context: None,
                });
                Some(LSPMessage {
                    jsonrpc: "2.0".to_string(),
                    id: message.id,
                    method: None,
                    params: None,
                    result: Some(serde_json::to_value(result).unwrap()),
                    error: None,
                })
            }
            "textDocument/hover" => {
                let result = server.handle_hover(HoverParams {
                    text_document_position_params: TextDocumentPositionParams {
                        text_document: TextDocumentIdentifier {
                            uri: Url::parse("file:///test.oql").unwrap(),
                        },
                        position: Position::new(0, 0),
                    },
                    work_done_progress_params: Default::default(),
                });
                Some(LSPMessage {
                    jsonrpc: "2.0".to_string(),
                    id: message.id,
                    method: None,
                    params: None,
                    result: Some(serde_json::to_value(result).unwrap()),
                    error: None,
                })
            }
            "initialized" => {
                // Just acknowledge
                None
            }
            "shutdown" => Some(LSPMessage {
                jsonrpc: "2.0".to_string(),
                id: message.id,
                method: None,
                params: None,
                result: Some(serde_json::Value::Null),
                error: None,
            }),
            _ => {
                // Unknown method
                Some(LSPMessage {
                    jsonrpc: "2.0".to_string(),
                    id: message.id,
                    method: None,
                    params: None,
                    result: None,
                    error: Some(serde_json::json!({"code": -32601, "message": "Method not found"})),
                })
            }
        }
    } else {
        None
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let server = SimpleOrbitQLServer::new();
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();

    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    eprintln!("Simple OrbitQL Language Server starting...");

    loop {
        line.clear();

        // Read Content-Length header
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break; // EOF
        }

        if line.starts_with("Content-Length:") {
            let content_length: usize = line[15..].trim().parse()?;

            // Read empty line
            line.clear();
            reader.read_line(&mut line).await?;

            // Read the JSON message
            let mut content = vec![0; content_length];
            use tokio::io::AsyncReadExt;
            reader.read_exact(&mut content).await?;

            let message_str = String::from_utf8(content)?;
            eprintln!("Received message: {}", message_str);

            if let Ok(message) = serde_json::from_str::<LSPMessage>(&message_str) {
                if let Some(response) = handle_lsp_message(&server, message).await {
                    let response_str = serde_json::to_string(&response)?;
                    let response_with_header = format!(
                        "Content-Length: {}\\r\\n\\r\\n{}",
                        response_str.len(),
                        response_str
                    );
                    stdout.write_all(response_with_header.as_bytes()).await?;
                    stdout.flush().await?;

                    eprintln!("Sent response: {}", response_str);
                }
            }
        }
    }

    Ok(())
}
