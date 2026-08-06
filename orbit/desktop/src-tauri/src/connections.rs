//! Connection management for Orbit Desktop.
//!
//! Two things are deliberately kept apart:
//!
//! * A [`Connection`] is the saved *description* of an endpoint — host, port,
//!   credentials, protocol. Descriptions are persisted by [`crate::storage`] and
//!   survive restarts.
//! * A [`DatabaseSession`] is a *live* handle opened from a description. Sessions
//!   never outlive the process.
//!
//! [`ConnectionManager`] owns both and opens sessions lazily, so a connection
//! saved in a previous run is usable on the next query without the user having
//! to recreate it. Because a session is held open across queries, protocol-level
//! session state (`SET`, temporary tables, open transactions, `USE <db>`,
//! `SELECT <n>` on Redis) behaves the way the user expects.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::queries::{ColumnInfo, QueryPayload};

/// How long to wait for a TCP connect / handshake when the user has not said.
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Types of database endpoints the desktop app can talk to.
///
/// The acronym spellings are deliberate: these names are the serialized
/// contract. They appear verbatim in `storage.json`, in the `ConnectionType`
/// enum in `src/types/index.ts`, and on the IPC boundary between them. Renaming
/// `CQL` to `Cql` would orphan every saved connection of that type.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionType {
    PostgreSQL,
    OrbitQL,
    Redis,
    MySQL,
    CQL,
    Cypher,
    AQL,
    FlightSQL,
    OrbitWire,
}

impl ConnectionType {
    /// Every variant, in the order the connection dialog should offer them.
    pub const ALL: [ConnectionType; 9] = [
        ConnectionType::PostgreSQL,
        ConnectionType::MySQL,
        ConnectionType::Redis,
        ConnectionType::CQL,
        ConnectionType::OrbitQL,
        ConnectionType::Cypher,
        ConnectionType::AQL,
        ConnectionType::FlightSQL,
        ConnectionType::OrbitWire,
    ];

    /// The port Orbit-RS listens on for this protocol by default.
    pub fn default_port(self) -> u16 {
        match self {
            ConnectionType::PostgreSQL => 5432,
            ConnectionType::MySQL => 3306,
            ConnectionType::Redis => 6379,
            ConnectionType::CQL => 9042,
            ConnectionType::OrbitQL
            | ConnectionType::Cypher
            | ConnectionType::AQL
            | ConnectionType::FlightSQL
            | ConnectionType::OrbitWire => 8080,
        }
    }

    /// Stable identifier used in persisted files and on the IPC boundary.
    pub fn as_str(self) -> &'static str {
        match self {
            ConnectionType::PostgreSQL => "PostgreSQL",
            ConnectionType::OrbitQL => "OrbitQL",
            ConnectionType::Redis => "Redis",
            ConnectionType::MySQL => "MySQL",
            ConnectionType::CQL => "CQL",
            ConnectionType::Cypher => "Cypher",
            ConnectionType::AQL => "AQL",
            ConnectionType::FlightSQL => "FlightSQL",
            ConnectionType::OrbitWire => "OrbitWire",
        }
    }

    /// Whether this protocol is served by Orbit-RS's native wire implementation.
    ///
    /// The HTTP-backed protocols currently reach `orbit-server`'s REST API,
    /// whose SQL and catalog handlers still return canned example rows. The UI
    /// uses this to label those results rather than presenting them as data.
    pub fn is_native_wire_protocol(self) -> bool {
        matches!(
            self,
            ConnectionType::PostgreSQL
                | ConnectionType::MySQL
                | ConnectionType::Redis
                | ConnectionType::CQL
        )
    }
}

impl fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ConnectionType {
    type Err = ConnectionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ConnectionType::ALL
            .into_iter()
            .find(|candidate| candidate.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| {
                ConnectionError::InvalidConfiguration(format!("unknown connection type: {s}"))
            })
    }
}

/// Everything needed to open a session, as supplied by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub name: String,
    pub connection_type: ConnectionType,
    pub host: String,
    pub port: u16,
    pub database: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl_mode: Option<String>,
    /// Connect/handshake timeout in milliseconds.
    pub connection_timeout: Option<u64>,
    #[serde(default)]
    pub additional_params: HashMap<String, String>,
}

impl ConnectionInfo {
    fn connect_timeout(&self) -> Duration {
        self.connection_timeout
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT)
    }

    fn http_base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    fn socket_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Reject a TLS request we cannot honour instead of silently connecting in
    /// the clear.
    ///
    /// # Errors
    /// Returns [`ConnectionError::InvalidConfiguration`] when `ssl_mode` asks
    /// for encryption, which this build does not implement.
    fn ensure_ssl_mode_supported(&self) -> Result<(), ConnectionError> {
        let mode = match self.ssl_mode.as_deref().map(str::trim) {
            None | Some("") => return Ok(()),
            Some(mode) => mode,
        };

        if matches!(
            mode.to_ascii_lowercase().as_str(),
            "disable" | "disabled" | "off" | "none" | "prefer" | "allow"
        ) {
            return Ok(());
        }

        Err(ConnectionError::InvalidConfiguration(format!(
            "ssl_mode '{mode}' requests an encrypted connection, which this build does not \
             support. Refusing rather than connecting in plaintext."
        )))
    }
}

/// Whether a saved connection currently has a live session behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Error(String),
}

/// A saved connection plus the usage counters shown in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub info: ConnectionInfo,
    pub status: ConnectionStatus,
    /// `None` when a persisted record carried a timestamp that would not parse.
    ///
    /// Deliberately not defaulted to "now": that would stamp every unreadable
    /// record with the time the app happened to start, which reads as a real
    /// creation date and is one nobody measured.
    pub created_at: Option<DateTime<Utc>>,
    pub last_used: Option<DateTime<Utc>>,
    pub query_count: u64,
}

/// Render an error together with everything that caused it.
///
/// Drivers routinely put the useful part in the source chain: `tokio_postgres`
/// reports a missing password as the top-level string "invalid configuration",
/// which tells the user nothing about what to change. The chain says
/// "invalid configuration: password missing".
fn describe(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let text = cause.to_string();
        // Drivers often repeat the outer message in the first source.
        if !message.contains(&text) {
            message.push_str(": ");
            message.push_str(&text);
        }
        source = cause.source();
    }
    message
}

/// Failures opening or using a connection.
#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    #[error("Connection not found: {0}")]
    ConnectionNotFound(String),
    #[error("Query failed: {0}")]
    QueryFailed(String),
    #[error("Timed out after {0:?}")]
    Timeout(Duration),
}

/// A live handle to one endpoint.
///
/// Implementations own whatever the protocol needs to keep a session alive and
/// are responsible for turning a raw response into a [`QueryPayload`].
#[async_trait]
pub trait DatabaseSession: Send + Sync {
    fn connection_type(&self) -> ConnectionType;

    /// Run one statement and shape the response for the results grid.
    ///
    /// # Errors
    /// Returns [`ConnectionError::QueryFailed`] when the server rejects the
    /// statement, or a transport variant when the session has dropped.
    async fn execute(&mut self, statement: &str) -> Result<QueryPayload, ConnectionError>;

    /// Cheap liveness probe used to decide whether a cached session is reusable.
    async fn ping(&mut self) -> Result<(), ConnectionError>;
}

/// A session shared between the manager and whoever is currently querying it.
///
/// The inner [`Mutex`] serialises statements on a single session — which is
/// what a database session requires — while leaving other connections free to
/// run concurrently.
pub type SessionHandle = Arc<Mutex<Box<dyn DatabaseSession>>>;

/// Owns saved connections and their live sessions.
#[derive(Default)]
pub struct ConnectionManager {
    connections: RwLock<HashMap<String, Connection>>,
    sessions: RwLock<HashMap<String, SessionHandle>>,
}

impl ConnectionManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a connection description without contacting the server.
    ///
    /// This is the startup path: descriptions restored from disk become
    /// queryable immediately, and the session opens on first use.
    pub async fn register(&self, connection: Connection) {
        self.connections
            .write()
            .await
            .insert(connection.id.clone(), connection);
    }

    /// Create a connection, verifying it can actually be opened first.
    ///
    /// # Errors
    /// Propagates whatever prevented the session from opening; nothing is
    /// stored when the endpoint is unreachable.
    pub async fn create_connection(
        &self,
        info: ConnectionInfo,
    ) -> Result<String, ConnectionError> {
        let session = open_session(&info).await?;
        let id = Uuid::new_v4().to_string();

        let connection = Connection {
            id: id.clone(),
            info,
            status: ConnectionStatus::Connected,
            created_at: Some(Utc::now()),
            last_used: None,
            query_count: 0,
        };

        self.connections
            .write()
            .await
            .insert(id.clone(), connection);
        self.sessions
            .write()
            .await
            .insert(id.clone(), Arc::new(Mutex::new(session)));

        Ok(id)
    }

    /// Open a throwaway session to check the details are usable.
    pub async fn test_connection(&self, info: &ConnectionInfo) -> ConnectionStatus {
        match open_session(info).await {
            Ok(_) => ConnectionStatus::Connected,
            Err(e) => ConnectionStatus::Error(e.to_string()),
        }
    }

    /// All saved connections, with `status` reflecting live session state.
    pub async fn list_connections(&self) -> Vec<Connection> {
        let sessions = self.sessions.read().await;
        self.connections
            .read()
            .await
            .values()
            .map(|connection| {
                let status = if sessions.contains_key(&connection.id) {
                    ConnectionStatus::Connected
                } else {
                    connection.status.clone()
                };
                Connection {
                    status,
                    ..connection.clone()
                }
            })
            .collect()
    }

    /// A single saved connection.
    pub async fn get_connection(&self, connection_id: &str) -> Option<Connection> {
        self.connections.read().await.get(connection_id).cloned()
    }

    /// Return a live session for `connection_id`, opening one if needed.
    ///
    /// A cached session that has since dropped is discarded and replaced rather
    /// than handed out, so a server restart costs one failed ping, not a dead
    /// connection the user has to notice and clear by hand.
    ///
    /// # Errors
    /// Returns [`ConnectionError::ConnectionNotFound`] for an unknown id, or
    /// the underlying failure when the endpoint cannot be reached.
    pub async fn session(&self, connection_id: &str) -> Result<SessionHandle, ConnectionError> {
        if let Some(handle) = self.sessions.read().await.get(connection_id).cloned() {
            let alive = handle.lock().await.ping().await.is_ok();
            if alive {
                return Ok(handle);
            }
            tracing::info!(connection_id, "cached session is dead, reopening");
            self.sessions.write().await.remove(connection_id);
        }

        let info = self
            .get_connection(connection_id)
            .await
            .ok_or_else(|| ConnectionError::ConnectionNotFound(connection_id.to_string()))?
            .info;

        // Opened outside the map lock so a slow handshake cannot stall queries
        // against other connections.
        let session = match open_session(&info).await {
            Ok(session) => session,
            Err(e) => {
                self.mark_error(connection_id, &e).await;
                return Err(e);
            }
        };

        let handle: SessionHandle = Arc::new(Mutex::new(session));
        let mut sessions = self.sessions.write().await;
        // Another task may have opened one while we were connecting; prefer
        // whichever landed first so both callers share a single session.
        let handle = sessions
            .entry(connection_id.to_string())
            .or_insert(handle)
            .clone();

        if let Some(connection) = self.connections.write().await.get_mut(connection_id) {
            connection.status = ConnectionStatus::Connected;
        }

        Ok(handle)
    }

    /// Drop the live session but keep the saved description.
    pub async fn disconnect(&self, connection_id: &str) {
        self.sessions.write().await.remove(connection_id);
        if let Some(connection) = self.connections.write().await.get_mut(connection_id) {
            connection.status = ConnectionStatus::Disconnected;
        }
    }

    /// Forget the connection entirely.
    pub async fn delete_connection(&self, connection_id: &str) {
        self.sessions.write().await.remove(connection_id);
        self.connections.write().await.remove(connection_id);
    }

    /// Record that a query ran, so the UI's counters mean something.
    pub async fn record_use(&self, connection_id: &str) {
        if let Some(connection) = self.connections.write().await.get_mut(connection_id) {
            connection.last_used = Some(Utc::now());
            connection.query_count += 1;
        }
    }

    async fn mark_error(&self, connection_id: &str, error: &ConnectionError) {
        if let Some(connection) = self.connections.write().await.get_mut(connection_id) {
            connection.status = ConnectionStatus::Error(error.to_string());
        }
    }
}

/// Open a live session for `info`, dispatching on protocol.
async fn open_session(
    info: &ConnectionInfo,
) -> Result<Box<dyn DatabaseSession>, ConnectionError> {
    info.ensure_ssl_mode_supported()?;

    match info.connection_type {
        ConnectionType::PostgreSQL => Ok(Box::new(PostgresSession::connect(info).await?)),
        ConnectionType::MySQL => Ok(Box::new(MySqlSession::connect(info).await?)),
        ConnectionType::Redis => Ok(Box::new(RedisSession::connect(info).await?)),
        ConnectionType::CQL => Ok(Box::new(TcpProbeSession::connect(info).await?)),
        ConnectionType::OrbitWire => Ok(Box::new(OrbitWireSession::connect(info).await?)),
        ConnectionType::OrbitQL
        | ConnectionType::Cypher
        | ConnectionType::AQL
        | ConnectionType::FlightSQL => Ok(Box::new(HttpSession::connect(info).await?)),
    }
}

// ---------------------------------------------------------------------------
// PostgreSQL
// ---------------------------------------------------------------------------

/// A PostgreSQL session held open across statements.
pub struct PostgresSession {
    client: tokio_postgres::Client,
    /// Whether the peer answers `Describe` well enough for `prepare()`.
    ///
    /// Real PostgreSQL does. `orbit-server`'s wire implementation replies to
    /// every `Describe` with `NoData` instead of a `ParameterDescription`, so
    /// `prepare()` fails there with "unexpected message from server" and the
    /// simple query protocol has to be used instead. Which one applies is
    /// decided once per session rather than per statement.
    extended_protocol: bool,
}

impl PostgresSession {
    async fn connect(info: &ConnectionInfo) -> Result<Self, ConnectionError> {
        let client = Self::open_client(info).await?;

        // Probe with a statement that cannot fail for any reason except an
        // unsupported extended protocol. The client stays usable either way —
        // a failed `prepare` does not disturb subsequent simple queries — so
        // this costs one round trip and no extra connection.
        let extended_protocol = client.prepare("SELECT 1").await.is_ok();
        if !extended_protocol {
            tracing::info!(
                host = %info.host,
                port = info.port,
                "peer does not support the extended query protocol; using simple queries"
            );
        }

        Ok(Self {
            client,
            extended_protocol,
        })
    }

    async fn open_client(info: &ConnectionInfo) -> Result<tokio_postgres::Client, ConnectionError> {
        // Built through `Config` rather than a connection string so that
        // passwords containing spaces, quotes or backslashes cannot corrupt
        // (or inject into) the parameter list.
        let mut config = tokio_postgres::Config::new();
        config
            .host(&info.host)
            .port(info.port)
            .connect_timeout(info.connect_timeout())
            .application_name("orbit-desktop");
        config.user(info.username.as_deref().unwrap_or("orbit"));
        if let Some(password) = &info.password {
            config.password(password);
        }
        if let Some(database) = &info.database {
            config.dbname(database);
        }

        let (client, connection) = config
            .connect(tokio_postgres::NoTls)
            .await
            .map_err(|e| ConnectionError::ConnectionFailed(describe(&e)))?;

        // The connection future drives the socket; it ends when the client is
        // dropped, which is what closes the session.
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::debug!("PostgreSQL connection closed: {e}");
            }
        });

        Ok(client)
    }

    /// Run a statement over the extended protocol, with server-declared types.
    async fn execute_extended(&self, statement: &str) -> Result<QueryPayload, ConnectionError> {
        // Preparing first tells us whether the statement returns a result set,
        // and gives real column types instead of guessing from a Debug string.
        let prepared = self
            .client
            .prepare(statement)
            .await
            .map_err(|e| ConnectionError::QueryFailed(describe(&e)))?;

        if prepared.columns().is_empty() {
            let affected = self
                .client
                .execute(&prepared, &[])
                .await
                .map_err(|e| ConnectionError::QueryFailed(describe(&e)))?;
            return Ok(QueryPayload::affected(affected));
        }

        let columns: Vec<ColumnInfo> = prepared
            .columns()
            .iter()
            .map(|column| ColumnInfo::new(column.name(), column.type_().name()))
            .collect();

        let rows = self
            .client
            .query(&prepared, &[])
            .await
            .map_err(|e| ConnectionError::QueryFailed(describe(&e)))?;

        let shaped = rows
            .iter()
            .map(|row| {
                columns
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        (column.name.clone(), postgres_value_to_json(row, index))
                    })
                    .collect()
            })
            .collect();

        Ok(QueryPayload::returned(columns, shaped))
    }

    /// Run a statement over the simple query protocol.
    ///
    /// Every value arrives as text and the protocol carries no type OIDs to the
    /// client, so columns are reported as `text`. That is what the wire actually
    /// said; guessing a richer type from the characters in a value would put an
    /// unverified claim in the type column.
    async fn execute_simple(&self, statement: &str) -> Result<QueryPayload, ConnectionError> {
        use tokio_postgres::SimpleQueryMessage;

        let messages = self
            .client
            .simple_query(statement)
            .await
            .map_err(|e| ConnectionError::QueryFailed(describe(&e)))?;

        let mut columns: Vec<ColumnInfo> = Vec::new();
        let mut rows: Vec<HashMap<String, serde_json::Value>> = Vec::new();
        let mut affected: Option<u64> = None;

        for message in messages {
            match message {
                SimpleQueryMessage::Row(row) => {
                    if columns.is_empty() {
                        columns = row
                            .columns()
                            .iter()
                            .map(|column| ColumnInfo::new(column.name(), "text"))
                            .collect();
                    }
                    rows.push(
                        columns
                            .iter()
                            .enumerate()
                            .map(|(index, column)| {
                                let value = row
                                    .get(index)
                                    .map(|text| serde_json::Value::String(text.to_string()))
                                    // A missing field here is SQL NULL: the
                                    // simple protocol sends those as absent.
                                    .unwrap_or(serde_json::Value::Null);
                                (column.name.clone(), value)
                            })
                            .collect(),
                    );
                }
                SimpleQueryMessage::CommandComplete(count) => {
                    affected = Some(affected.unwrap_or(0) + count);
                }
                // The enum is non_exhaustive; anything new carries no rows.
                _ => {}
            }
        }

        if !columns.is_empty() {
            return Ok(QueryPayload::returned(columns, rows));
        }

        match affected {
            Some(count) => Ok(QueryPayload::affected(count)),
            None => Ok(QueryPayload {
                columns: Vec::new(),
                rows: Vec::new(),
                outcome: crate::queries::StatementOutcome::Completed,
            }),
        }
    }
}

#[async_trait]
impl DatabaseSession for PostgresSession {
    fn connection_type(&self) -> ConnectionType {
        ConnectionType::PostgreSQL
    }

    async fn execute(&mut self, statement: &str) -> Result<QueryPayload, ConnectionError> {
        if self.extended_protocol {
            self.execute_extended(statement).await
        } else {
            self.execute_simple(statement).await
        }
    }

    async fn ping(&mut self) -> Result<(), ConnectionError> {
        if self.client.is_closed() {
            return Err(ConnectionError::NetworkError("session closed".to_string()));
        }
        self.client
            .simple_query("")
            .await
            .map(|_| ())
            .map_err(|e| ConnectionError::NetworkError(e.to_string()))
    }
}

/// Decode one PostgreSQL column into JSON, keeping SQL `NULL` distinct from
/// "this client could not decode it".
fn postgres_value_to_json(row: &tokio_postgres::Row, index: usize) -> serde_json::Value {
    use serde_json::Value;

    /// `Ok(Some)` decoded, `Ok(None)` SQL NULL, `Err` not decodable here.
    fn decode<'a, T>(row: &'a tokio_postgres::Row, index: usize) -> Result<Option<T>, ()>
    where
        T: tokio_postgres::types::FromSql<'a>,
    {
        row.try_get::<_, Option<T>>(index).map_err(|_| ())
    }

    fn number(value: f64) -> Value {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(value.to_string()))
    }

    let column_type = row.columns()[index].type_();

    let decoded = match column_type.name() {
        "bool" => decode::<bool>(row, index).map(|v| v.map(Value::Bool)),
        "int2" => decode::<i16>(row, index).map(|v| v.map(|n| Value::Number(n.into()))),
        "int4" => decode::<i32>(row, index).map(|v| v.map(|n| Value::Number(n.into()))),
        "int8" => decode::<i64>(row, index).map(|v| v.map(|n| Value::Number(n.into()))),
        "oid" => decode::<u32>(row, index).map(|v| v.map(|n| Value::Number(n.into()))),
        "float4" => decode::<f32>(row, index).map(|v| v.map(|n| number(f64::from(n)))),
        "float8" => decode::<f64>(row, index).map(|v| v.map(number)),
        // Rendered as text: f64 cannot hold every NUMERIC exactly, and silently
        // rounding a monetary column is worse than making the caller parse it.
        "numeric" => decode::<rust_decimal::Decimal>(row, index)
            .map(|v| v.map(|d| Value::String(d.to_string()))),
        "uuid" => decode::<uuid::Uuid>(row, index).map(|v| v.map(|u| Value::String(u.to_string()))),
        "json" | "jsonb" => decode::<Value>(row, index),
        "timestamptz" => decode::<DateTime<Utc>>(row, index)
            .map(|v| v.map(|t| Value::String(t.to_rfc3339()))),
        "timestamp" => decode::<chrono::NaiveDateTime>(row, index)
            .map(|v| v.map(|t| Value::String(t.to_string()))),
        "date" => decode::<chrono::NaiveDate>(row, index)
            .map(|v| v.map(|t| Value::String(t.to_string()))),
        "time" => decode::<chrono::NaiveTime>(row, index)
            .map(|v| v.map(|t| Value::String(t.to_string()))),
        "bytea" => decode::<Vec<u8>>(row, index)
            .map(|v| v.map(|bytes| Value::String(format!("\\x{}", hex_encode(&bytes))))),
        _ => decode::<String>(row, index).map(|v| v.map(Value::String)),
    };

    match decoded {
        Ok(Some(value)) => value,
        Ok(None) => Value::Null,
        // Not null, but this client has no decoder. Saying so beats rendering
        // it as NULL, which would read as "the database has no value here".
        Err(()) => Value::String(format!("<undecoded {}>", column_type.name())),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    use fmt::Write as _;
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut acc, byte| {
        // Writing to a String cannot fail; the Result is discarded deliberately.
        let _ = write!(acc, "{byte:02x}");
        acc
    })
}

// ---------------------------------------------------------------------------
// MySQL
// ---------------------------------------------------------------------------

/// A MySQL session held open across statements.
pub struct MySqlSession {
    conn: mysql_async::Conn,
}

impl MySqlSession {
    async fn connect(info: &ConnectionInfo) -> Result<Self, ConnectionError> {
        let opts = mysql_async::OptsBuilder::default()
            .ip_or_hostname(info.host.clone())
            .tcp_port(info.port)
            .user(info.username.clone())
            .pass(info.password.clone())
            .db_name(info.database.clone());

        let conn = tokio::time::timeout(
            info.connect_timeout(),
            mysql_async::Conn::new(opts),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(info.connect_timeout()))?
        .map_err(|e| ConnectionError::ConnectionFailed(e.to_string()))?;

        Ok(Self { conn })
    }
}

#[async_trait]
impl DatabaseSession for MySqlSession {
    fn connection_type(&self) -> ConnectionType {
        ConnectionType::MySQL
    }

    async fn execute(&mut self, statement: &str) -> Result<QueryPayload, ConnectionError> {
        use mysql_async::prelude::Queryable;

        let mut result = self
            .conn
            .query_iter(statement)
            .await
            .map_err(|e| ConnectionError::QueryFailed(e.to_string()))?;

        // Column metadata has to be taken before the result set is consumed.
        let columns: Vec<ColumnInfo> = result
            .columns_ref()
            .iter()
            .map(|column| {
                ColumnInfo::new(
                    column.name_str().as_ref(),
                    format!("{:?}", column.column_type()).to_lowercase(),
                )
            })
            .collect();

        if columns.is_empty() {
            let affected = result.affected_rows();
            result
                .drop_result()
                .await
                .map_err(|e| ConnectionError::QueryFailed(e.to_string()))?;
            return Ok(QueryPayload::affected(affected));
        }

        let rows: Vec<mysql_async::Row> = result
            .collect()
            .await
            .map_err(|e| ConnectionError::QueryFailed(e.to_string()))?;

        let shaped = rows
            .into_iter()
            .map(|row| {
                columns
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        let value = row
                            .as_ref(index)
                            .map(mysql_value_to_json)
                            .unwrap_or(serde_json::Value::Null);
                        (column.name.clone(), value)
                    })
                    .collect()
            })
            .collect();

        Ok(QueryPayload::returned(columns, shaped))
    }

    async fn ping(&mut self) -> Result<(), ConnectionError> {
        use mysql_async::prelude::Queryable;

        self.conn
            .ping()
            .await
            .map_err(|e| ConnectionError::NetworkError(e.to_string()))
    }
}

/// Decode one MySQL column into JSON.
fn mysql_value_to_json(value: &mysql_async::Value) -> serde_json::Value {
    use mysql_async::Value as My;
    use serde_json::Value;

    match value {
        My::NULL => Value::Null,
        My::Int(n) => Value::Number((*n).into()),
        My::UInt(n) => Value::Number((*n).into()),
        My::Float(n) => serde_json::Number::from_f64(f64::from(*n))
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(n.to_string())),
        My::Double(n) => serde_json::Number::from_f64(*n)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(n.to_string())),
        My::Bytes(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) => Value::String(text.to_string()),
            Err(_) => Value::String(format!("\\x{}", hex_encode(bytes))),
        },
        My::Date(year, month, day, hour, minute, second, micros) => Value::String(format!(
            "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{micros:06}"
        )),
        My::Time(negative, days, hours, minutes, seconds, micros) => {
            let sign = if *negative { "-" } else { "" };
            Value::String(format!(
                "{sign}{days}d {hours:02}:{minutes:02}:{seconds:02}.{micros:06}"
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Redis
// ---------------------------------------------------------------------------

/// A Redis session held open across commands, so `SELECT <db>` and `MULTI`
/// apply to subsequent commands the way `redis-cli` behaves.
pub struct RedisSession {
    conn: redis::aio::MultiplexedConnection,
}

impl RedisSession {
    async fn connect(info: &ConnectionInfo) -> Result<Self, ConnectionError> {
        let mut url = format!("redis://{}:{}", info.host, info.port);
        if let Some(database) = info.database.as_deref().filter(|d| !d.is_empty()) {
            url.push('/');
            url.push_str(database);
        }

        let client = redis::Client::open(url)
            .map_err(|e| ConnectionError::InvalidConfiguration(e.to_string()))?;

        let mut conn = tokio::time::timeout(
            info.connect_timeout(),
            client.get_multiplexed_async_connection(),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(info.connect_timeout()))?
        .map_err(|e| ConnectionError::ConnectionFailed(e.to_string()))?;

        if let Some(password) = info.password.as_deref().filter(|p| !p.is_empty()) {
            let mut auth = redis::cmd("AUTH");
            if let Some(user) = info.username.as_deref().filter(|u| !u.is_empty()) {
                auth.arg(user);
            }
            auth.arg(password);
            auth.query_async::<redis::Value>(&mut conn)
                .await
                .map_err(|e| ConnectionError::ConnectionFailed(format!("AUTH failed: {e}")))?;
        }

        Ok(Self { conn })
    }
}

#[async_trait]
impl DatabaseSession for RedisSession {
    fn connection_type(&self) -> ConnectionType {
        ConnectionType::Redis
    }

    async fn execute(&mut self, statement: &str) -> Result<QueryPayload, ConnectionError> {
        let args = split_redis_command(statement);
        let (name, rest) = args
            .split_first()
            .ok_or_else(|| ConnectionError::QueryFailed("empty Redis command".to_string()))?;

        let mut command = redis::cmd(&name.to_uppercase());
        for arg in rest {
            command.arg(arg.as_str());
        }

        let value = command
            .query_async::<redis::Value>(&mut self.conn)
            .await
            .map_err(|e| ConnectionError::QueryFailed(e.to_string()))?;

        Ok(QueryPayload::single_value(
            "result",
            "redis",
            redis_value_to_json(value),
        ))
    }

    async fn ping(&mut self) -> Result<(), ConnectionError> {
        redis::cmd("PING")
            .query_async::<redis::Value>(&mut self.conn)
            .await
            .map(|_| ())
            .map_err(|e| ConnectionError::NetworkError(e.to_string()))
    }
}

/// Split a Redis command line, honouring single and double quoted arguments so
/// that `SET greeting "hello world"` is two arguments rather than three.
fn split_redis_command(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;

    for ch in line.trim().chars() {
        match (quote, ch) {
            (Some(q), c) if c == q => {
                quote = None;
            }
            (Some(_), c) => current.push(c),
            (None, c @ ('"' | '\'')) => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started || !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (None, c) => current.push(c),
        }
    }

    if started || !current.is_empty() {
        args.push(current);
    }

    args
}

/// Convert a RESP value into JSON, recursing into aggregates rather than
/// falling back to a `Debug` rendering.
fn redis_value_to_json(value: redis::Value) -> serde_json::Value {
    use redis::Value as Resp;
    use serde_json::Value;

    match value {
        Resp::Nil => Value::Null,
        Resp::Int(n) => Value::Number(n.into()),
        Resp::BulkString(bytes) => match String::from_utf8(bytes) {
            Ok(text) => Value::String(text),
            Err(e) => Value::String(format!("\\x{}", hex_encode(e.as_bytes()))),
        },
        Resp::Array(values) | Resp::Set(values) => {
            Value::Array(values.into_iter().map(redis_value_to_json).collect())
        }
        Resp::SimpleString(text) => Value::String(text),
        Resp::Okay => Value::String("OK".to_string()),
        Resp::Map(pairs) => Value::Object(
            pairs
                .into_iter()
                .map(|(key, value)| {
                    let key = match redis_value_to_json(key) {
                        Value::String(text) => text,
                        other => other.to_string(),
                    };
                    (key, redis_value_to_json(value))
                })
                .collect(),
        ),
        Resp::Attribute { data, .. } => redis_value_to_json(*data),
        Resp::Double(n) => serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(n.to_string())),
        Resp::Boolean(b) => Value::Bool(b),
        Resp::VerbatimString { text, .. } => Value::String(text),
        Resp::BigNumber(n) => Value::String(n.to_string()),
        Resp::Push { kind, data } => Value::Object(
            [
                ("kind".to_string(), Value::String(format!("{kind:?}"))),
                (
                    "data".to_string(),
                    Value::Array(data.into_iter().map(redis_value_to_json).collect()),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        Resp::ServerError(error) => Value::String(format!("{error:?}")),
    }
}

// ---------------------------------------------------------------------------
// HTTP-backed protocols
// ---------------------------------------------------------------------------

/// A session for the protocols Orbit-RS exposes over HTTP.
///
/// The reqwest client is kept so that keep-alive applies across statements.
pub struct HttpSession {
    client: reqwest::Client,
    base_url: String,
    flavor: ConnectionType,
    username: Option<String>,
    password: Option<String>,
}

impl HttpSession {
    async fn connect(info: &ConnectionInfo) -> Result<Self, ConnectionError> {
        let client = reqwest::Client::builder()
            .connect_timeout(info.connect_timeout())
            .build()
            .map_err(|e| ConnectionError::InvalidConfiguration(e.to_string()))?;

        let session = Self {
            client,
            base_url: info.http_base_url(),
            flavor: info.connection_type,
            username: info.username.clone(),
            password: info.password.clone(),
        };

        session.probe().await?;
        Ok(session)
    }

    /// Endpoint and body for one statement, per protocol.
    fn request_for(&self, statement: &str) -> (String, serde_json::Value) {
        match self.flavor {
            ConnectionType::Cypher => (
                format!("{}/db/data/transaction/commit", self.base_url),
                serde_json::json!({ "statements": [{ "statement": statement }] }),
            ),
            ConnectionType::AQL => (
                format!("{}/_api/cursor", self.base_url),
                serde_json::json!({ "query": statement, "count": true }),
            ),
            ConnectionType::FlightSQL => (
                format!("{}/api/v1/flight/sql", self.base_url),
                serde_json::json!({ "query": statement }),
            ),
            _ => (
                format!("{}/api/v1/sql", self.base_url),
                serde_json::json!({ "query": statement }),
            ),
        }
    }

    /// Confirm something is listening and answering before declaring success.
    async fn probe(&self) -> Result<(), ConnectionError> {
        let url = match self.flavor {
            ConnectionType::AQL => format!("{}/_api/version", self.base_url),
            _ => format!("{}/health", self.base_url),
        };

        let response = self
            .authenticated(self.client.get(&url))
            .send()
            .await
            .map_err(|e| ConnectionError::NetworkError(e.to_string()))?;

        // A 404 still proves a server answered; the health path merely differs.
        if response.status().is_success() || response.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(ConnectionError::ConnectionFailed(format!(
                "health probe returned HTTP {}",
                response.status()
            )))
        }
    }

    fn authenticated(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match (&self.username, &self.password) {
            (Some(user), password) => builder.basic_auth(user, password.as_ref()),
            (None, _) => builder,
        }
    }
}

#[async_trait]
impl DatabaseSession for HttpSession {
    fn connection_type(&self) -> ConnectionType {
        self.flavor
    }

    async fn execute(&mut self, statement: &str) -> Result<QueryPayload, ConnectionError> {
        let (url, body) = self.request_for(statement);

        let response = self
            .authenticated(self.client.post(&url))
            .json(&body)
            .send()
            .await
            .map_err(|e| ConnectionError::NetworkError(e.to_string()))?;

        let status = response.status();
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ConnectionError::QueryFailed(format!("HTTP {status}: {e}")))?;

        if !status.is_success() {
            return Err(ConnectionError::QueryFailed(format!(
                "HTTP {status}: {payload}"
            )));
        }

        Ok(QueryPayload::from_json(&payload))
    }

    async fn ping(&mut self) -> Result<(), ConnectionError> {
        self.probe().await
    }
}

// ---------------------------------------------------------------------------
// Raw TCP protocols
// ---------------------------------------------------------------------------

/// A protocol we can prove is listening but cannot yet speak.
///
/// Used for CQL: the desktop app has no CQL binary-protocol client, so it
/// verifies reachability and then refuses statements rather than pretending to
/// have run them.
pub struct TcpProbeSession {
    addr: String,
    flavor: ConnectionType,
    timeout: Duration,
}

impl TcpProbeSession {
    async fn connect(info: &ConnectionInfo) -> Result<Self, ConnectionError> {
        let session = Self {
            addr: info.socket_addr(),
            flavor: info.connection_type,
            timeout: info.connect_timeout(),
        };
        session.probe().await?;
        Ok(session)
    }

    async fn probe(&self) -> Result<(), ConnectionError> {
        tokio::time::timeout(self.timeout, tokio::net::TcpStream::connect(&self.addr))
            .await
            .map_err(|_| ConnectionError::Timeout(self.timeout))?
            .map(|_| ())
            .map_err(|e| ConnectionError::ConnectionFailed(e.to_string()))
    }
}

#[async_trait]
impl DatabaseSession for TcpProbeSession {
    fn connection_type(&self) -> ConnectionType {
        self.flavor
    }

    async fn execute(&mut self, _statement: &str) -> Result<QueryPayload, ConnectionError> {
        Err(ConnectionError::QueryFailed(format!(
            "{} statements cannot be executed from the desktop app yet: the {0} binary protocol \
             has no client here. The endpoint at {} is reachable — use cqlsh against it, or \
             connect over PostgreSQL/MySQL instead.",
            self.flavor, self.addr
        )))
    }

    async fn ping(&mut self) -> Result<(), ConnectionError> {
        self.probe().await
    }
}

/// OrbitWire: handshake over TCP to prove the server speaks the protocol, then
/// carry statements over the REST endpoint on the same host.
pub struct OrbitWireSession {
    http: HttpSession,
}

impl OrbitWireSession {
    /// `"ORBT"` plus protocol version 1.
    const HANDSHAKE: [u8; 5] = [0x4F, 0x52, 0x42, 0x54, 0x01];

    async fn connect(info: &ConnectionInfo) -> Result<Self, ConnectionError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let timeout = info.connect_timeout();
        let mut stream =
            tokio::time::timeout(timeout, tokio::net::TcpStream::connect(info.socket_addr()))
                .await
                .map_err(|_| ConnectionError::Timeout(timeout))?
                .map_err(|e| ConnectionError::ConnectionFailed(e.to_string()))?;

        stream
            .write_all(&Self::HANDSHAKE)
            .await
            .map_err(|e| ConnectionError::NetworkError(e.to_string()))?;

        let mut response = [0u8; 5];
        tokio::time::timeout(timeout, stream.read_exact(&mut response))
            .await
            .map_err(|_| ConnectionError::Timeout(timeout))?
            .map_err(|e| ConnectionError::NetworkError(e.to_string()))?;

        if &response[..4] != b"ORBT" {
            return Err(ConnectionError::ConnectionFailed(
                "peer did not answer the OrbitWire handshake".to_string(),
            ));
        }

        // Statements travel over REST; `http_port` says where, defaulting to the
        // REST listener rather than assuming it shares the wire port.
        let rest_port = info
            .additional_params
            .get("http_port")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(8080);

        let http = HttpSession::connect(&ConnectionInfo {
            port: rest_port,
            connection_type: ConnectionType::OrbitWire,
            ..info.clone()
        })
        .await?;

        Ok(Self { http })
    }
}

#[async_trait]
impl DatabaseSession for OrbitWireSession {
    fn connection_type(&self) -> ConnectionType {
        ConnectionType::OrbitWire
    }

    async fn execute(&mut self, statement: &str) -> Result<QueryPayload, ConnectionError> {
        self.http.execute(statement).await
    }

    async fn ping(&mut self) -> Result<(), ConnectionError> {
        self.http.ping().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_type_round_trips_through_its_string_form() {
        for variant in ConnectionType::ALL {
            let parsed: ConnectionType = variant
                .as_str()
                .parse()
                .expect("every variant's own string must parse back");
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn connection_type_parsing_is_case_insensitive_and_rejects_junk() {
        assert_eq!(
            "postgresql".parse::<ConnectionType>().ok(),
            Some(ConnectionType::PostgreSQL)
        );
        assert!("Postgres".parse::<ConnectionType>().is_err());
    }

    #[test]
    fn default_ports_match_the_documented_protocol_ports() {
        assert_eq!(ConnectionType::PostgreSQL.default_port(), 5432);
        assert_eq!(ConnectionType::MySQL.default_port(), 3306);
        assert_eq!(ConnectionType::Redis.default_port(), 6379);
        assert_eq!(ConnectionType::CQL.default_port(), 9042);
        assert_eq!(ConnectionType::OrbitQL.default_port(), 8080);
    }

    fn info_with_ssl(mode: Option<&str>) -> ConnectionInfo {
        ConnectionInfo {
            name: "test".to_string(),
            connection_type: ConnectionType::PostgreSQL,
            host: "localhost".to_string(),
            port: 5432,
            database: None,
            username: None,
            password: None,
            ssl_mode: mode.map(str::to_string),
            connection_timeout: None,
            additional_params: HashMap::new(),
        }
    }

    #[test]
    fn plaintext_ssl_modes_are_accepted() {
        for mode in [None, Some(""), Some("disable"), Some("prefer"), Some("allow")] {
            assert!(info_with_ssl(mode).ensure_ssl_mode_supported().is_ok());
        }
    }

    #[test]
    fn encrypting_ssl_modes_are_refused_rather_than_downgraded() {
        for mode in ["require", "verify-ca", "verify-full"] {
            assert!(info_with_ssl(Some(mode)).ensure_ssl_mode_supported().is_err());
        }
    }

    #[test]
    fn connect_timeout_falls_back_to_the_default_when_unset() {
        assert_eq!(
            info_with_ssl(None).connect_timeout(),
            DEFAULT_CONNECT_TIMEOUT
        );
        let mut info = info_with_ssl(None);
        info.connection_timeout = Some(250);
        assert_eq!(info.connect_timeout(), Duration::from_millis(250));
    }

    #[test]
    fn redis_command_splitting_keeps_quoted_arguments_whole() {
        assert_eq!(
            split_redis_command("SET greeting \"hello world\""),
            vec!["SET", "greeting", "hello world"]
        );
        assert_eq!(
            split_redis_command("  GET   key  "),
            vec!["GET", "key"]
        );
        assert_eq!(
            split_redis_command("SET empty \"\""),
            vec!["SET", "empty", ""]
        );
        assert!(split_redis_command("   ").is_empty());
    }

    #[test]
    fn redis_aggregates_convert_without_debug_fallbacks() {
        use redis::Value as Resp;
        let nested = Resp::Array(vec![
            Resp::Int(1),
            Resp::BulkString(b"two".to_vec()),
            Resp::Array(vec![Resp::Okay]),
        ]);
        assert_eq!(
            redis_value_to_json(nested),
            serde_json::json!([1, "two", ["OK"]])
        );
    }

    #[test]
    fn hex_encoding_pads_every_byte() {
        assert_eq!(hex_encode(&[0x00, 0x0f, 0xff]), "000fff");
    }
}

/// Tests that need a running `orbit-server`.
///
/// A green unit-test run says nothing about whether the app can actually reach
/// Orbit, so these drive the real session types against real listeners. They
/// are `#[ignore]`d because they need a server:
///
/// ```text
/// ./target/debug/orbit-server --dev-mode --data-dir /tmp/orbit-verify &
/// cargo test --manifest-path orbit/desktop/src-tauri/Cargo.toml -- --ignored --test-threads=1
/// ```
#[cfg(test)]
mod live_tests {
    use super::*;

    /// `orbit-server` auto-registers an unknown user with the password set to
    /// the username, so these credentials work against a fresh dev server.
    const USER: &str = "orbit";

    fn info(connection_type: ConnectionType, port: u16) -> ConnectionInfo {
        ConnectionInfo {
            name: format!("live-{connection_type}"),
            connection_type,
            host: "127.0.0.1".to_string(),
            port,
            database: None,
            username: Some(USER.to_string()),
            password: Some(USER.to_string()),
            ssl_mode: None,
            connection_timeout: Some(5_000),
            additional_params: HashMap::new(),
        }
    }

    #[tokio::test]
    #[ignore = "requires a running orbit-server on 5432"]
    async fn a_connection_failure_reports_the_underlying_cause() {
        // The driver's own message here is the useless "invalid configuration";
        // the reason lives one level down in the source chain.
        let mut without_password = info(ConnectionType::PostgreSQL, 5432);
        without_password.password = None;

        let message = match PostgresSession::connect(&without_password).await {
            Ok(_) => panic!("the server asks for a password, so this must fail"),
            Err(e) => e.to_string(),
        };
        assert!(
            message.contains("password"),
            "the message must name the cause, got: {message}"
        );
    }

    #[tokio::test]
    #[ignore = "requires a running orbit-server on 5432"]
    async fn postgres_session_runs_a_statement_and_shapes_the_rows() {
        let mut session = PostgresSession::connect(&info(ConnectionType::PostgreSQL, 5432))
            .await
            .expect("orbit-server should accept a PostgreSQL connection");

        session.ping().await.expect("ping should succeed");

        let payload = session
            .execute("SELECT 1 AS one")
            .await
            .expect("SELECT 1 should execute");

        assert_eq!(payload.columns.len(), 1, "one column expected");
        assert_eq!(payload.columns[0].name, "one");
        assert_eq!(
            payload.outcome,
            crate::queries::StatementOutcome::Returned { rows: 1 }
        );

        // Both protocol paths must surface the value. The extended path decodes
        // int4 to a JSON number; the simple path can only report the text the
        // wire carried, and says so in the column type rather than guessing.
        let value = payload.rows[0].get("one").expect("the column must be present");
        assert!(
            *value == serde_json::json!(1) || *value == serde_json::json!("1"),
            "expected the value 1 in either form, got {value}"
        );
    }

    #[tokio::test]
    #[ignore = "requires a running orbit-server on 5432"]
    async fn a_session_survives_across_statements() {
        let mut session = PostgresSession::connect(&info(ConnectionType::PostgreSQL, 5432))
            .await
            .expect("connect");

        // Three statements on one session: if the manager were reconnecting per
        // query this would still pass, but the session would be a new one each
        // time and any SET or temp table would vanish.
        for expected in 1..=3 {
            let payload = session
                .execute(&format!("SELECT {expected} AS n"))
                .await
                .expect("statement should execute on the reused session");
            let value = payload.rows[0].get("n").expect("column n");
            assert!(
                *value == serde_json::json!(expected)
                    || *value == serde_json::json!(expected.to_string()),
                "expected {expected} in either form, got {value}"
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires a running orbit-server on 6379"]
    async fn redis_session_runs_commands_and_decodes_replies() {
        let mut session = RedisSession::connect(&info(ConnectionType::Redis, 6379))
            .await
            .expect("orbit-server should accept a Redis connection");

        session.ping().await.expect("PING should succeed");

        let payload = session.execute("PING").await.expect("PING should execute");
        assert_eq!(payload.columns.len(), 1);
        assert!(
            !payload.rows.is_empty(),
            "a Redis reply should produce one row"
        );
    }

    #[tokio::test]
    #[ignore = "requires a running orbit-server on 5432"]
    async fn the_manager_reopens_a_session_for_a_restored_connection() {
        let manager = ConnectionManager::new();

        // Exactly the startup path: register a description with no live session,
        // as `restore_connections` does for every connection loaded from disk.
        let connection = Connection {
            id: "restored".to_string(),
            info: info(ConnectionType::PostgreSQL, 5432),
            status: ConnectionStatus::Disconnected,
            created_at: None,
            last_used: None,
            query_count: 0,
        };
        manager.register(connection).await;

        let session = manager
            .session("restored")
            .await
            .expect("a restored connection must be usable without being recreated");

        let payload = session
            .lock()
            .await
            .execute("SELECT 1 AS one")
            .await
            .expect("query on the lazily opened session");
        let value = payload.rows[0].get("one").expect("column one");
        assert!(
            *value == serde_json::json!(1) || *value == serde_json::json!("1"),
            "expected 1 in either form, got {value}"
        );

        let listed = manager.list_connections().await;
        assert_eq!(listed[0].status, ConnectionStatus::Connected);
    }
}
