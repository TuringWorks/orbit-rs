//! PostgreSQL wire protocol handler

use bytes::{BufMut, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tracing::{debug, error, info, warn};

use super::auth::{configured_auth_method, AuthManager, AuthMethod, ScramAuth, UserStore};
use super::messages::{
    type_oids, AuthenticationResponse, BackendMessage, FieldDescription, FrontendMessage,
    PasswordMessageKind, TransactionStatus,
};
use super::notifications::{NotificationHub, SessionNotifications};
use super::query_engine::{QueryEngine, QueryResult};
use crate::protocols::error::{ProtocolError, ProtocolResult};

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Initial,
    Authenticating,
    Authenticated,
    Ready,
    InTransaction,
    Closed,
}

/// The fixed 11-byte header of a binary `COPY` stream.
const COPY_BINARY_SIGNATURE: &[u8] = b"PGCOPY\n\xff\r\n\0";

/// A savepoint: how far each undo log had grown when it was taken.
struct Savepoint {
    name: String,
    inserts: HashMap<String, usize>,
    pre_images: HashMap<String, usize>,
    /// Whole-table copies for the statements that leave no row-level record.
    tables: HashMap<String, Vec<super::persistent_storage::TableRow>>,
}

/// A distinct backend id for each session.
///
/// PostgreSQL gives every backend its own process id; this used to report
/// `std::process::id()`, the same value for every connection. Two things read
/// it and both were wrong: the cancel registry is keyed by it, so a map with
/// one slot meant only the newest connection could ever be cancelled, and
/// `NOTIFY` reports it so a listener can tell its own notifications apart —
/// with one shared id every notification looked self-sent.
fn next_process_id() -> i32 {
    static NEXT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1);
    // Wrapping keeps it positive: a negative id would be a valid i32 but not
    // something a client would expect from a pid.
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .rem_euclid(i32::MAX)
}

/// PostgreSQL wire protocol handler
pub struct PostgresWireProtocol {
    state: ConnectionState,
    username: Option<String>,
    database: Option<String>,
    parameters: HashMap<String, String>,
    query_engine: Arc<QueryEngine>,
    process_id: i32,
    /// PostgreSQL 18 (protocol 3.2): Variable-length cancel key (4-256 bytes)
    /// Default: 4 bytes for backward compatibility with protocol 3.0
    secret_key: Vec<u8>,
    prepared_statements: HashMap<String, String>,
    /// Parameter type OIDs supplied by `Parse`, per prepared statement.
    ///
    /// Needed to answer `Describe(Statement)`, which must report one type per
    /// parameter before the row description.
    statement_param_types: HashMap<String, Vec<i32>>,
    /// Whether the message being handled belongs to the extended protocol.
    ///
    /// Only there does an error start skipping: a simple query synchronises
    /// with its own `ReadyForQuery` and never sends `Sync`, so treating its
    /// failures the same way left the connection discarding everything the
    /// client sent next — including the queries that would have cleared it.
    handling_extended: bool,
    /// Whether an extended-protocol message has failed since the last `Sync`.
    ///
    /// The protocol requires everything after an error to be discarded until
    /// the client synchronises. Without this the statements queued behind a
    /// failure still ran, so a client pipelining writes had later ones applied
    /// when it expected them skipped.
    skip_until_sync: bool,
    portals: HashMap<String, (String, Vec<Option<bytes::Bytes>>)>,
    /// Result format codes requested by `Bind`, per portal.
    ///
    /// A client that asked for binary results cannot read text ones: it decodes
    /// by width and fails with "failed to fill whole buffer". Ignoring these
    /// codes only appeared to work while every column was advertised as text.
    portal_result_formats: HashMap<String, Vec<i16>>,
    /// Column type OIDs most recently described for a statement, so `Execute`
    /// knows how to encode each value.
    statement_columns: HashMap<String, Vec<i32>>,
    /// Whether the COPY being started came from a simple query.
    copy_in_is_simple: bool,
    /// The `COPY ... FROM STDIN` currently in progress, if any.
    ///
    /// While set, the session is in copy-in mode and the client is streaming
    /// CopyData messages rather than ordinary queries.
    copy_in: Option<CopyInState>,
    /// Shared LISTEN/NOTIFY registry, and this session's end of it.
    notifications: Arc<NotificationHub>,
    session_notifications: SessionNotifications,
    /// Rows of a partially fetched portal, so a second `Execute` resumes rather
    /// than re-running the statement.
    portal_rows: HashMap<String, PortalRows>,
    auth_manager: AuthManager,
    scram_auth: Option<ScramAuth>,
    /// The half-finished GSSAPI handshake, once one has been asked for.
    ///
    /// Its presence is also what tells the message parser that a `'p'` message
    /// on this connection is a token rather than a password.
    #[cfg(feature = "gssapi")]
    gss: Option<super::gssapi::Acceptor>,
    /// Writes issued inside the current transaction block.
    writes_in_transaction: u64,
    /// Contents of each table as it stood when the transaction block first
    /// wrote to it.
    ///
    /// Storage applies writes as they run, so undoing them means putting the
    /// table back. A snapshot is taken once per table per block, before its
    /// first write, and discarded on COMMIT.
    transaction_snapshots: HashMap<String, Vec<super::persistent_storage::TableRow>>,
    /// Rows this session added in the open block, per table.
    ///
    /// Undo removes exactly these rather than restoring a copy of the table,
    /// which would take another session's committed rows with it.
    transaction_inserts: HashMap<String, Vec<super::persistent_storage::TableRow>>,
    /// Rows this session changed or removed, as they stood beforehand.
    transaction_pre_images: HashMap<String, Vec<super::persistent_storage::TableRow>>,
    /// The transaction this session has open, if any.
    ///
    /// Rows written inside it are stamped with this id and stay invisible to
    /// other sessions until it ends.
    transaction_id: Option<super::query_engine::TransactionContext>,
    /// The isolation level asked for, which decides whether a block reads
    /// through a snapshot or sees each commit as it lands.
    snapshot_isolation: bool,
    /// Whether the block must also fail if what it read moved underneath it.
    serializable: bool,
    /// Whether this connection asked for the replication protocol at startup.
    ///
    /// A replication connection speaks a small command set instead of SQL,
    /// which is why the mode has to be known before the first query.
    replication: bool,
    /// The slot the current replication stream belongs to, if it named one.
    replication_slot_name: Option<String>,
    /// The output plugin that slot was created with, which decides the payload
    /// format: `pgoutput` is the binary protocol a real subscriber speaks.
    replication_plugin: String,
    /// Tables already described to the subscriber, so a `Relation` message is
    /// sent once rather than before every row.
    announced_relations: std::collections::HashSet<String>,
    /// Table ids handed out for this stream.
    relation_ids: HashMap<String, i32>,
    /// The transaction a `Begin` has been sent for and not yet closed.
    replication_open_transaction: Option<u64>,
    /// Whether the subscriber asked for binary values rather than text.
    replication_binary: bool,
    /// Set when another connection asks to cancel this session's work.
    cancelled: Option<Arc<std::sync::atomic::AtomicBool>>,
    /// The change stream a `START_REPLICATION` opened, if any.
    replication_stream: Option<tokio::sync::broadcast::Receiver<super::query_engine::ChangeRecord>>,
    /// Whether this session has asked for immediate constraint checking.
    ///
    /// `SET CONSTRAINTS ALL IMMEDIATE` makes deferrable keys behave as if they
    /// were not deferrable for the rest of the transaction, so a later write
    /// fails at the statement rather than at `COMMIT`.
    constraints_immediate: bool,
    /// Open savepoints, innermost last.
    ///
    /// Each records how far the undo logs had grown when it was taken, so
    /// rolling back to it undoes exactly the writes made afterwards.
    savepoints: Vec<Savepoint>,
    /// Whether this session is inside a transaction block, and whether that
    /// block has already failed.
    ///
    /// Reported in every `ReadyForQuery`. It used to be hardcoded to `Idle`,
    /// which told drivers no transaction was ever open — so a driver could not
    /// tell a committed statement from one queued in an aborted block.
    transaction: TransactionState,
}

/// An in-progress `COPY ... FROM STDIN`.
struct CopyInState {
    /// Whether the stream is in the binary format rather than text.
    binary: bool,
    /// Whether the text stream is CSV rather than tab-separated.
    ///
    /// Read as tabs, a CSV line arrived as one field and the load failed with
    /// a column-count mismatch.
    csv: bool,
    /// Bytes of a binary stream not yet forming a whole tuple.
    pending: BytesMut,
    /// Whether the fixed binary header has been consumed.
    header_seen: bool,
    /// Whether the copy was started by a simple query, which owes the client a
    /// ReadyForQuery when the stream ends.
    simple_protocol: bool,
    table: String,
    /// Column names the data is being loaded into, in order.
    columns: Vec<String>,
    /// PostgreSQL type OID of each column, for decoding a binary stream.
    column_type_oids: Vec<i32>,
    /// Whether each column takes an unquoted literal, by position.
    ///
    /// A COPY field is text on the wire but must be written into the statement
    /// the way its column expects: quoting `9` for an integer column makes the
    /// insert fail, and the failure arrives mid-stream where the client is not
    /// expecting a message at all.
    numeric_columns: Vec<bool>,
    /// Bytes received but not yet terminated by a newline.
    ///
    /// CopyData messages are chunks of a byte stream, not rows: a row can be
    /// split across two of them.
    partial: Vec<u8>,
    rows: u64,
    /// First row failure, reported when the stream ends.
    failure: Option<String>,
}

/// A portal's result set and how much of it has been delivered.
struct PortalRows {
    columns: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
    sent: usize,
}

/// Transaction state of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionState {
    /// No transaction block open; each statement commits on its own.
    Idle,
    /// Inside a transaction block that is still good.
    Open,
    /// Inside a transaction block that has hit an error. Every statement is
    /// rejected until ROLLBACK.
    Failed,
}

/// Result of processing data in the connection loop
#[derive(Debug)]
enum ConnectionLoopResult {
    Continue,
    ClientDisconnected,
    ClientTerminated,
}

/// Result of processing a single message
#[derive(Debug)]
enum MessageResult {
    Continue,
    Terminate,
    Error(ProtocolError),
}

impl PostgresWireProtocol {
    /// Create a new PostgreSQL protocol handler
    pub fn new() -> Self {
        // Initialize user store with a default user
        let user_store = UserStore::new();
        // TODO: In a real app, we wouldn't add this user here or we'd load from config
        let auth_method = configured_auth_method();
        let auth_manager = AuthManager::new(auth_method, user_store);

        Self {
            state: ConnectionState::Initial,
            username: None,
            database: None,
            parameters: HashMap::new(),
            query_engine: Arc::new(QueryEngine::new()),
            process_id: next_process_id(),
            secret_key: Self::random_secret_key(),
            prepared_statements: HashMap::new(),
            statement_param_types: HashMap::new(),
            handling_extended: false,
            skip_until_sync: false,
            portals: HashMap::new(),
            portal_result_formats: HashMap::new(),
            statement_columns: HashMap::new(),
            portal_rows: HashMap::new(),
            auth_manager,
            scram_auth: None,
            #[cfg(feature = "gssapi")]
            gss: None,
            transaction: TransactionState::Idle,
            writes_in_transaction: 0,
            transaction_snapshots: HashMap::new(),
            transaction_id: None,
            snapshot_isolation: false,
            serializable: false,
            replication: false,
            replication_slot_name: None,
            replication_plugin: "orbit_json".to_string(),
            announced_relations: std::collections::HashSet::new(),
            relation_ids: HashMap::new(),
            replication_open_transaction: None,
            replication_binary: false,
            cancelled: None,
            replication_stream: None,
            constraints_immediate: false,
            transaction_inserts: HashMap::new(),
            transaction_pre_images: HashMap::new(),
            savepoints: Vec::new(),
            copy_in: None,
            copy_in_is_simple: false,
            notifications: NotificationHub::new(),
            session_notifications: SessionNotifications::new(),
        }
    }

    /// Create a new PostgreSQL protocol handler with custom query engine
    pub fn new_with_query_engine(query_engine: Arc<QueryEngine>) -> Self {
        tracing::debug!("wire protocol session created with a custom query engine");
        let user_store = UserStore::new();
        let auth_method = configured_auth_method();
        let auth_manager = AuthManager::new(auth_method, user_store);

        Self {
            state: ConnectionState::Initial,
            username: None,
            database: None,
            parameters: HashMap::new(),
            query_engine,
            process_id: next_process_id(),
            secret_key: Self::random_secret_key(),
            prepared_statements: HashMap::new(),
            statement_param_types: HashMap::new(),
            handling_extended: false,
            skip_until_sync: false,
            portals: HashMap::new(),
            portal_result_formats: HashMap::new(),
            statement_columns: HashMap::new(),
            portal_rows: HashMap::new(),
            auth_manager,
            scram_auth: None,
            #[cfg(feature = "gssapi")]
            gss: None,
            transaction: TransactionState::Idle,
            writes_in_transaction: 0,
            transaction_snapshots: HashMap::new(),
            transaction_id: None,
            snapshot_isolation: false,
            serializable: false,
            replication: false,
            replication_slot_name: None,
            replication_plugin: "orbit_json".to_string(),
            announced_relations: std::collections::HashSet::new(),
            relation_ids: HashMap::new(),
            replication_open_transaction: None,
            replication_binary: false,
            cancelled: None,
            replication_stream: None,
            constraints_immediate: false,
            transaction_inserts: HashMap::new(),
            transaction_pre_images: HashMap::new(),
            savepoints: Vec::new(),
            copy_in: None,
            copy_in_is_simple: false,
            notifications: NotificationHub::new(),
            session_notifications: SessionNotifications::new(),
        }
    }

    /// Share a notification registry with the other sessions on this server.
    ///
    /// Without this each connection gets its own hub, so `NOTIFY` on one
    /// connection can never reach a `LISTEN` on another — which is the only
    /// thing the feature is for.
    #[must_use]
    pub fn with_notification_hub(mut self, hub: Arc<NotificationHub>) -> Self {
        self.notifications = hub;
        self
    }

    /// Transaction status to report in `ReadyForQuery`.
    fn transaction_status(&self) -> TransactionStatus {
        match self.transaction {
            TransactionState::Idle => TransactionStatus::Idle,
            TransactionState::Open => TransactionStatus::InTransaction,
            TransactionState::Failed => TransactionStatus::InFailedTransaction,
        }
    }

    /// Update the transaction state from a statement about to run.
    ///
    /// Recognises the transaction-control statements themselves; everything
    /// else leaves the state alone.
    fn note_statement(&mut self, sql: &str) {
        // One message may carry several statements. Reading only the first
        // word of the whole thing meant `BEGIN; ...; COMMIT` was seen as a
        // `BEGIN` alone, and the session was left holding a transaction open
        // that the client had already committed.
        let statements = super::query_engine::split_statements(sql);
        if statements.len() > 1 {
            for statement in statements {
                self.note_one_statement(&statement);
            }
            return;
        }
        self.note_one_statement(sql);
    }

    /// Update the transaction state from a single statement.
    fn note_one_statement(&mut self, sql: &str) {
        let head: String = sql
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
            .to_uppercase();

        match head.as_str() {
            "BEGIN" | "START" => {
                self.transaction_id = Some(super::query_engine::begin_transaction_at(
                    self.snapshot_isolation,
                    self.serializable,
                ));
                self.transaction = TransactionState::Open;
                self.writes_in_transaction = 0;
            }
            "COMMIT" | "END" | "ROLLBACK" | "ABORT" => {
                // Ending the transaction is what makes its rows visible to
                // everyone else; a rolled-back block's rows are removed by the
                // undo log before this point.
                if let Some(context) = self.transaction_id.take() {
                    // The subscriber's `Begin`/`Commit` pair closes here, so a
                    // multi-statement block arrives as one transaction rather
                    // than as several.
                    if matches!(head.as_str(), "COMMIT" | "END") {
                        QueryEngine::publish_transaction_end(context.id);
                    }
                    super::query_engine::end_transaction(context.id);
                }
                self.snapshot_isolation = false;
                self.serializable = false;
                self.transaction = TransactionState::Idle;
                self.writes_in_transaction = 0;
                self.savepoints.clear();
                self.constraints_immediate = false;
            }
            // Writes are counted so a later ROLLBACK can report what it cannot
            // undo.
            "INSERT" | "UPDATE" | "DELETE" | "MERGE" | "COPY" | "TRUNCATE"
                if self.transaction != TransactionState::Idle =>
            {
                self.writes_in_transaction += 1;
            }
            _ => {}
        }
    }

    /// Snapshot the table a statement is about to write, if a transaction
    /// block is open and this is its first write to that table.
    ///
    /// Taken before the statement runs, because afterwards the previous
    /// contents are gone.
    async fn snapshot_before_write(&mut self, sql: &str) {
        if self.transaction == TransactionState::Idle {
            return;
        }
        let Some(table) = QueryEngine::write_target_table(sql) else {
            return;
        };

        // `COPY ... FROM STDIN` records nothing here: its rows arrive later and
        // each one is inserted as its own statement, which lands in the undo
        // log on its own.
        // The whole-table copy below is the fallback for anything else that
        // cannot be expressed as a set of rows.
        let head: String = sql
            .trim_start()
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect::<String>()
            .to_uppercase();

        if head == "COPY" {
            return;
        }

        if matches!(head.as_str(), "INSERT") {
            match self.query_engine.rows_an_insert_adds(sql).await {
                Ok(Some(rows)) => {
                    self.transaction_inserts
                        .entry(table)
                        .or_default()
                        .extend(rows);
                    return;
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("could not record inserted rows for rollback: {e}"),
            }
        } else if head == "TRUNCATE" {
            // Everything currently in the table is what TRUNCATE removes, so
            // every row is its own pre-image. Rolling back re-inserts only
            // those, leaving rows another session added afterwards alone.
            match self.query_engine.snapshot_table(&table).await {
                Ok(Some(rows)) => {
                    self.transaction_pre_images
                        .entry(table)
                        .or_default()
                        .extend(rows);
                    return;
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("could not record pre-images for rollback: {e}"),
            }
        } else if matches!(head.as_str(), "UPDATE" | "DELETE") {
            match self.query_engine.rows_a_statement_will_change(sql).await {
                Ok(Some(rows)) => {
                    self.transaction_pre_images
                        .entry(table)
                        .or_default()
                        .extend(rows);
                    return;
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("could not record pre-images for rollback: {e}"),
            }
        }

        if self.transaction_snapshots.contains_key(&table) {
            return;
        }
        match self.query_engine.snapshot_table(&table).await {
            Ok(Some(rows)) => {
                self.transaction_snapshots.insert(table, rows);
            }
            Ok(None) => {}
            // Recorded but not fatal: the statement still runs, and the
            // rollback will report that it could not restore this table.
            Err(e) => tracing::warn!("could not snapshot '{table}' for rollback: {e}"),
        }
    }

    /// Put every snapshotted table back, undoing the block's writes.
    ///
    /// Returns the tables that could not be restored.
    async fn restore_snapshots(&mut self) -> Vec<String> {
        let inserts = std::mem::take(&mut self.transaction_inserts);
        let pre_images = std::mem::take(&mut self.transaction_pre_images);
        let snapshots = std::mem::take(&mut self.transaction_snapshots);
        let mut failed = Vec::new();

        // Row-scoped undo first: it touches only what this session wrote.
        let tables: std::collections::BTreeSet<String> =
            inserts.keys().chain(pre_images.keys()).cloned().collect();
        for table in tables {
            let added = inserts.get(&table).map(Vec::as_slice).unwrap_or_default();
            let before = pre_images
                .get(&table)
                .map(Vec::as_slice)
                .unwrap_or_default();
            if let Err(e) = self
                .query_engine
                .undo_session_writes(&table, added, before)
                .await
            {
                tracing::error!("rollback could not undo writes to '{table}': {e}");
                failed.push(table);
            }
        }

        // Whole-table restore only for the statements that left no row-level
        // record; this one can revert a concurrent session's writes, which is
        // why it is the fallback.
        for (table, rows) in snapshots {
            if let Err(e) = self.query_engine.restore_table(&table, rows).await {
                tracing::error!("rollback could not restore '{table}': {e}");
                failed.push(table);
            }
        }
        failed
    }

    /// Apply a transaction-control statement's effect on the undo snapshots.
    ///
    /// `ROLLBACK` puts every table the block wrote back to how it stood before
    /// its first write, which is what makes the block atomic. `COMMIT` simply
    /// drops the snapshots.
    ///
    /// This gives atomicity, not isolation: writes are visible to other
    /// sessions as they happen, and a concurrent writer's changes to the same
    /// table would be reverted along with this block's.
    async fn apply_transaction_control(&mut self, query: &str, buf: &mut BytesMut) {
        let head: String = query
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_uppercase();

        match head.as_str() {
            "ROLLBACK" | "ABORT" => {
                // Rows this block marked deleted are put back by clearing the
                // mark; the row-level undo handles everything it wrote.
                if let Some(id) = self.transaction_id.as_ref().map(|c| c.id) {
                    let tables = self.tables_written();
                    if let Err(e) = self.query_engine.restore_deleted(id, &tables).await {
                        tracing::error!("could not restore deletes for transaction {id}: {e}");
                    }
                }
                let failed = self.restore_snapshots().await;
                if failed.is_empty() {
                    return;
                }
                // Partly undone: saying so beats reporting a clean rollback.
                let mut fields = HashMap::new();
                fields.insert(b'S', "WARNING".to_string());
                fields.insert(b'C', "25000".to_string());
                fields.insert(
                    b'M',
                    format!(
                        "ROLLBACK could not restore {}: those changes remain applied",
                        failed.join(", ")
                    ),
                );
                BackendMessage::NoticeResponse { fields }.encode(buf);
            }
            // Deferred constraints are checked now, before the block's writes
            // are allowed to stand. A failure here has to undo them, as
            // PostgreSQL does when a deferred check fails at COMMIT.
            "COMMIT" | "END" => {
                if let Err(e) = self
                    .query_engine
                    .check_deferred_constraints(&self.tables_written())
                    .await
                {
                    let failed = self.restore_snapshots().await;
                    self.send_error_for(buf, &e);
                    if !failed.is_empty() {
                        tracing::error!("could not undo after a deferred failure: {failed:?}");
                    }
                    self.savepoints.clear();
                    return;
                }
            }
            _ => {}
        }

        match head.as_str() {
            "COMMIT" | "END" => {
                // A serializable block that read something another transaction
                // has since written cannot be serialized after it: PostgreSQL
                // fails it here rather than committing a result no serial
                // order could produce.
                if let Some(context) = self.transaction_id.clone() {
                    if let (Some(snapshot), Some(reads)) =
                        (context.snapshot.as_ref(), context.reads.as_ref())
                    {
                        let tables: Vec<String> = reads
                            .lock()
                            .map(|reads| reads.iter().cloned().collect())
                            .unwrap_or_default();
                        match self
                            .query_engine
                            .serialization_conflict(context.id, snapshot, &tables)
                            .await
                        {
                            Ok(Some(table)) => {
                                let failed = self.restore_snapshots().await;
                                if !failed.is_empty() {
                                    tracing::error!("could not undo after a conflict: {failed:?}");
                                }
                                let mut fields = HashMap::new();
                                fields.insert(b'S', "ERROR".to_string());
                                fields.insert(b'C', "40001".to_string());
                                fields.insert(
                                    b'M',
                                    format!(
                                        "could not serialize access due to concurrent update on \"{table}\""
                                    ),
                                );
                                BackendMessage::ErrorResponse { fields }.encode(buf);
                                self.savepoints.clear();
                                return;
                            }
                            Ok(None) => {}
                            Err(e) => tracing::error!("serialization check failed: {e}"),
                        }
                    }
                }

                // A committed delete removes its rows for good.
                if let Some(id) = self.transaction_id.as_ref().map(|c| c.id) {
                    let tables = self.tables_written();
                    if let Err(e) = self.query_engine.purge_deleted(id, &tables).await {
                        tracing::error!("could not purge deletes for transaction {id}: {e}");
                    }
                }
                self.transaction_snapshots.clear();
                self.transaction_inserts.clear();
                self.transaction_pre_images.clear();
            }
            _ => {}
        }
    }

    /// Handle `SET <name> = <value>` and `SHOW <name>`.
    ///
    /// Returns `None` when the statement is neither, `Ok(None)` when a value
    /// was stored, and `Ok(Some(result))` with the row `SHOW` reports.
    fn handle_session_parameter(
        &mut self,
        query: &str,
    ) -> Option<Result<Option<QueryResult>, String>> {
        let trimmed = query.trim().trim_end_matches(';').trim();
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let head = words.first()?.to_uppercase();

        if head == "SHOW" {
            let name = words.get(1)?.to_lowercase();
            // `SHOW ALL` reports every parameter, one row each, as psql's
            // `\\set`-style introspection expects.
            if name == "all" {
                let mut rows: Vec<Vec<Option<String>>> = self
                    .parameters
                    .iter()
                    .map(|(key, value)| vec![Some(key.clone()), Some(value.clone())])
                    .collect();
                rows.sort();
                return Some(Ok(Some(QueryResult::Select {
                    columns: vec!["name".to_string(), "setting".to_string()],
                    rows,
                })));
            }
            let value = self.parameters.get(&name).cloned().unwrap_or_default();
            return Some(Ok(Some(QueryResult::Select {
                columns: vec![name],
                rows: vec![vec![Some(value)]],
            })));
        }

        if head != "SET" {
            return None;
        }
        // `SET TRANSACTION ...`, `SET SESSION ...` and friends are not simple
        // parameter assignments; leave them to the engine.
        let name = words.get(1)?.to_lowercase();
        if matches!(
            name.as_str(),
            "transaction" | "session" | "local" | "constraints"
        ) {
            return None;
        }

        let rest = trimmed
            .split_once('=')
            .map(|(_, value)| value.trim())
            .or_else(|| {
                words
                    .get(2)
                    .filter(|word| word.eq_ignore_ascii_case("TO"))
                    .and_then(|_| words.get(3))
                    .copied()
            })?;

        // A parameter value is written as a SQL literal; the stored value is
        // the string it denotes.
        let value = rest.trim().trim_matches('\'').trim_matches('"').to_string();
        self.parameters.insert(name, value);
        Some(Ok(None))
    }

    /// The tables this transaction block has written.
    ///
    /// Taken from the undo logs, which already record exactly that.
    fn tables_written(&self) -> Vec<String> {
        self.transaction_inserts
            .keys()
            .chain(self.transaction_pre_images.keys())
            .chain(self.transaction_snapshots.keys())
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Handle a walsender command.
    ///
    /// Returns `None` when the command is not one, so a replication connection
    /// can still run ordinary SQL, which `replication=database` allows.
    async fn handle_replication_command(
        &mut self,
        query: &str,
        buf: &mut BytesMut,
    ) -> Option<ProtocolResult<()>> {
        use super::messages::type_oids;

        let trimmed = query.trim().trim_end_matches(';').trim();
        let upper = trimmed.to_uppercase();

        let finish = |buf: &mut BytesMut, tag: &str, status: super::messages::TransactionStatus| {
            BackendMessage::CommandComplete {
                tag: tag.to_string(),
            }
            .encode(buf);
            BackendMessage::ReadyForQuery { status }.encode(buf);
        };

        if upper == "IDENTIFY_SYSTEM" {
            let columns = ["systemid", "timeline", "xlogpos", "dbname"];
            BackendMessage::RowDescription {
                fields: columns
                    .iter()
                    .map(|name| super::messages::FieldDescription {
                        name: (*name).to_string(),
                        table_oid: 0,
                        column_id: 0,
                        type_oid: type_oids::TEXT,
                        type_size: -1,
                        type_modifier: -1,
                        format: 0,
                    })
                    .collect(),
            }
            .encode(buf);
            BackendMessage::DataRow {
                values: vec![
                    Some(bytes::Bytes::from(Self::system_identifier())),
                    Some(bytes::Bytes::from_static(b"1")),
                    Some(bytes::Bytes::from(Self::current_lsn())),
                    Some(bytes::Bytes::from(
                        self.database.clone().unwrap_or_else(|| "orbit".to_string()),
                    )),
                ],
            }
            .encode(buf);
            finish(buf, "IDENTIFY_SYSTEM", self.transaction_status());
            return Some(Ok(()));
        }

        if upper.starts_with("TIMELINE_HISTORY") {
            // One timeline, so there is no history file to send. Saying so is
            // the answer; inventing a file would be worse.
            self.send_error(
                buf,
                "requested timeline is the current one, which has no history file",
            );
            BackendMessage::ReadyForQuery {
                status: self.transaction_status(),
            }
            .encode(buf);
            return Some(Ok(()));
        }

        if upper.starts_with("CREATE_REPLICATION_SLOT") {
            let name = trimmed.split_whitespace().nth(1).unwrap_or("slot");
            let plugin = trimmed.split_whitespace().last().unwrap_or("orbit_json");
            let stored = name.trim_matches('"').to_string();
            if let Err(e) = self
                .query_engine
                .create_replication_slot(&stored, plugin)
                .await
            {
                self.send_error_for(buf, &e);
                BackendMessage::ReadyForQuery {
                    status: self.transaction_status(),
                }
                .encode(buf);
                return Some(Ok(()));
            }
            BackendMessage::RowDescription {
                fields: [
                    "slot_name",
                    "consistent_point",
                    "snapshot_name",
                    "output_plugin",
                ]
                .iter()
                .map(|name| super::messages::FieldDescription {
                    name: (*name).to_string(),
                    table_oid: 0,
                    column_id: 0,
                    type_oid: type_oids::TEXT,
                    type_size: -1,
                    type_modifier: -1,
                    format: 0,
                })
                .collect(),
            }
            .encode(buf);
            BackendMessage::DataRow {
                values: vec![
                    Some(bytes::Bytes::from(name.trim_matches('"').to_string())),
                    Some(bytes::Bytes::from(Self::current_lsn())),
                    None,
                    Some(bytes::Bytes::from(plugin.to_string())),
                ],
            }
            .encode(buf);
            finish(buf, "CREATE_REPLICATION_SLOT", self.transaction_status());
            return Some(Ok(()));
        }

        if upper.starts_with("DROP_REPLICATION_SLOT") {
            if let Some(name) = trimmed.split_whitespace().nth(1) {
                let _ = self
                    .query_engine
                    .drop_replication_slot(name.trim_matches('"'))
                    .await;
            }
            finish(buf, "DROP_REPLICATION_SLOT", self.transaction_status());
            return Some(Ok(()));
        }

        if upper.starts_with("START_REPLICATION") {
            // Physical replication streams raw WAL. This server has no
            // PostgreSQL WAL to stream, and answering a physical request with
            // logical frames would be a wrong answer rather than a missing
            // feature — the standby would parse change JSON as WAL records.
            if upper.contains("PHYSICAL") {
                // `0A000` is `feature_not_supported`, which is what this is.
                // Reported as `XX000` a client could not tell a feature this
                // server does not have from a backend that fell over.
                self.send_error_for(
                    buf,
                    &ProtocolError::SqlState {
                        code: "0A000",
                        message: "physical replication is not supported; use \
                                  START_REPLICATION SLOT <name> LOGICAL"
                            .to_string(),
                    },
                );
                BackendMessage::ReadyForQuery {
                    status: self.transaction_status(),
                }
                .encode(buf);
                return Some(Ok(()));
            }

            // `(proto_version '1', binary 'true')` — the options a subscriber
            // passes to the output plugin.
            self.replication_binary = trimmed
                .split_once('(')
                .map(|(_, options)| options.to_lowercase())
                .is_some_and(|options| options.contains("binary") && options.contains("true"));

            let words: Vec<&str> = trimmed.split_whitespace().collect();
            let slot = words
                .iter()
                .position(|word| word.eq_ignore_ascii_case("SLOT"))
                .and_then(|at| words.get(at + 1))
                .map(|name| name.trim_matches('"').to_string());

            // The position the replica asks to resume from, written as an LSN.
            let requested = words
                .iter()
                .find(|word| word.contains('/'))
                .and_then(|word| {
                    let (high, low) = word.split_once('/')?;
                    let high = u64::from_str_radix(high, 16).ok()?;
                    let low = u64::from_str_radix(low, 16).ok()?;
                    Some((high << 32) | low)
                })
                .filter(|position| *position > 0);

            // A slot's confirmed position is used when the replica names none,
            // which is what makes the slot worth persisting.
            let resume = match (requested, slot.as_ref()) {
                (Some(position), _) => Some(position),
                (None, Some(name)) => self
                    .query_engine
                    .replication_slot(name)
                    .await
                    .ok()
                    .flatten()
                    .map(|(_, position)| position),
                (None, None) => None,
            };

            // A named slot has to exist. Streaming from one that was dropped —
            // or invalidated for falling too far behind — would look like a
            // healthy subscription that silently starts from nowhere.
            if let Some(name) = slot.as_ref() {
                match self.query_engine.replication_slot(name).await {
                    Ok(Some(_)) => {}
                    _ => {
                        self.send_error(
                            buf,
                            &format!("replication slot \"{name}\" does not exist"),
                        );
                        BackendMessage::ReadyForQuery {
                            status: self.transaction_status(),
                        }
                        .encode(buf);
                        return Some(Ok(()));
                    }
                }
            }

            // Subscribing before replaying means a change written in between
            // is queued rather than lost.
            let live = super::query_engine::subscribe_to_changes();

            // The stream is both directions at once: changes go out, standby
            // status updates come back.
            BackendMessage::CopyBothResponse {
                format: 0,
                column_formats: Vec::new(),
            }
            .encode(buf);

            if let Some(position) = resume {
                // The in-memory window answers a recent request without a
                // read; the durable log answers one that reaches further back,
                // including after a restart when the window is empty.
                let replayed = match super::query_engine::changes_since(position) {
                    Some(records) => records,
                    None => match self.query_engine.logged_changes_since(position).await {
                        Ok(records) if !records.is_empty() => records,
                        // Nothing after this position anywhere: the replica is
                        // already current, so it just starts streaming.
                        Ok(_) if position >= super::query_engine::latest_change_position() => {
                            Vec::new()
                        }
                        // The position is genuinely behind what is retained.
                        // Saying so beats a stream with a hole in it.
                        _ => {
                            self.send_error(
                                buf,
                                "requested WAL position is older than the retained history",
                            );
                            return Some(Ok(()));
                        }
                    },
                };
                for record in replayed {
                    self.send_change(&record, buf);
                }
            }

            if let Some(name) = slot.as_ref() {
                if let Ok(Some((plugin, _))) = self.query_engine.replication_slot(name).await {
                    self.replication_plugin = plugin;
                }
            }
            self.replication_slot_name = slot;
            self.replication_stream = Some(live);
            return Some(Ok(()));
        }

        None
    }

    /// A stable identifier for this server, as `IDENTIFY_SYSTEM` reports it.
    fn system_identifier() -> String {
        // Derived from the process, so two servers do not claim to be one.
        format!("{}", std::process::id() as u64 + 7_000_000_000_000_000_000)
    }

    /// The write position, rendered the way PostgreSQL writes an LSN.
    fn current_lsn() -> String {
        let position = super::query_engine::latest_change_position();
        format!("{:X}/{:X}", position >> 32, position & 0xFFFF_FFFF)
    }

    /// Whether a statement ends the current transaction block.
    fn ends_transaction(query: &str) -> bool {
        let head: String = query
            .trim_start()
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect::<String>()
            .to_uppercase();
        matches!(head.as_str(), "ROLLBACK" | "ABORT" | "COMMIT" | "END")
    }

    /// Handle `SAVEPOINT`, `ROLLBACK TO SAVEPOINT` and `RELEASE`.
    ///
    /// Returns `None` when the statement is none of those. A savepoint is a
    /// second layer of undo inside the block: without it `ROLLBACK TO` fell
    /// through to a plain `ROLLBACK` and discarded the whole block.
    async fn handle_savepoint(&mut self, query: &str) -> Option<Result<String, String>> {
        let trimmed = query.trim().trim_end_matches(';').trim();
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let upper: Vec<String> = words.iter().map(|w| w.to_uppercase()).collect();
        let name_after = |index: usize| words.get(index).map(|w| w.to_lowercase());

        // `ROLLBACK TO [SAVEPOINT] name`
        if upper.first().is_some_and(|w| w == "ROLLBACK") && upper.get(1).is_some_and(|w| w == "TO")
        {
            let at = if upper.get(2).is_some_and(|w| w == "SAVEPOINT") {
                3
            } else {
                2
            };
            let Some(name) = name_after(at) else {
                return Some(Err("ROLLBACK TO requires a savepoint name".to_string()));
            };
            let Some(index) = self.savepoints.iter().rposition(|s| s.name == name) else {
                return Some(Err(format!("savepoint \"{name}\" does not exist")));
            };

            let inserts_at = self.savepoints[index].inserts.clone();
            let pre_images_at = self.savepoints[index].pre_images.clone();
            let tables_at = self.savepoints[index].tables.clone();
            self.savepoints.truncate(index + 1);

            // Undo only what was written after the savepoint: the tail of each
            // undo log beyond the length it had when the savepoint was taken.
            let touched: std::collections::BTreeSet<String> = self
                .transaction_inserts
                .keys()
                .chain(self.transaction_pre_images.keys())
                .cloned()
                .collect();
            for table in touched {
                let kept_inserts = inserts_at.get(&table).copied().unwrap_or(0);
                let kept_pre_images = pre_images_at.get(&table).copied().unwrap_or(0);
                let added: Vec<_> = self
                    .transaction_inserts
                    .get(&table)
                    .map(|rows| rows[kept_inserts.min(rows.len())..].to_vec())
                    .unwrap_or_default();
                let before: Vec<_> = self
                    .transaction_pre_images
                    .get(&table)
                    .map(|rows| rows[kept_pre_images.min(rows.len())..].to_vec())
                    .unwrap_or_default();

                if let Err(e) = self
                    .query_engine
                    .undo_session_writes(&table, &added, &before)
                    .await
                {
                    return Some(Err(format!(
                        "could not roll back '{table}' to savepoint: {e}"
                    )));
                }

                if let Some(rows) = self.transaction_inserts.get_mut(&table) {
                    let keep = kept_inserts.min(rows.len());
                    rows.truncate(keep);
                }
                if let Some(rows) = self.transaction_pre_images.get_mut(&table) {
                    let keep = kept_pre_images.min(rows.len());
                    rows.truncate(keep);
                }
            }

            // A table written by a statement with no row-level record goes back
            // to the copy taken at the savepoint.
            for (table, rows) in tables_at {
                if let Err(e) = self.query_engine.restore_table(&table, rows).await {
                    return Some(Err(format!(
                        "could not roll back '{table}' to savepoint: {e}"
                    )));
                }
            }

            // The block continues, and a failure before this point is undone.
            if self.transaction == TransactionState::Failed {
                self.transaction = TransactionState::Open;
            }
            return Some(Ok("ROLLBACK".to_string()));
        }

        if upper.first().is_some_and(|w| w == "SAVEPOINT") {
            let Some(name) = name_after(1) else {
                return Some(Err("SAVEPOINT requires a name".to_string()));
            };
            if self.transaction == TransactionState::Idle {
                return Some(Err(
                    "SAVEPOINT can only be used in transaction blocks".to_string()
                ));
            }

            let mut tables = HashMap::new();
            let written: Vec<String> = self.transaction_snapshots.keys().cloned().collect();
            for table in written {
                match self.query_engine.snapshot_table(&table).await {
                    Ok(Some(rows)) => {
                        tables.insert(table, rows);
                    }
                    Ok(None) => {}
                    Err(e) => return Some(Err(format!("could not record savepoint: {e}"))),
                }
            }
            self.savepoints.push(Savepoint {
                name,
                inserts: self
                    .transaction_inserts
                    .iter()
                    .map(|(table, rows)| (table.clone(), rows.len()))
                    .collect(),
                pre_images: self
                    .transaction_pre_images
                    .iter()
                    .map(|(table, rows)| (table.clone(), rows.len()))
                    .collect(),
                tables,
            });
            return Some(Ok("SAVEPOINT".to_string()));
        }

        if upper.first().is_some_and(|w| w == "RELEASE") {
            let at = if upper.get(1).is_some_and(|w| w == "SAVEPOINT") {
                2
            } else {
                1
            };
            let Some(name) = name_after(at) else {
                return Some(Err("RELEASE requires a savepoint name".to_string()));
            };
            let Some(index) = self.savepoints.iter().rposition(|s| s.name == name) else {
                return Some(Err(format!("savepoint \"{name}\" does not exist")));
            };
            self.savepoints.truncate(index);
            return Some(Ok("RELEASE".to_string()));
        }

        None
    }

    /// Record that a statement failed.
    ///
    /// Inside a transaction block this poisons it: PostgreSQL rejects every
    /// later statement until the block is rolled back.
    fn note_failure(&mut self) {
        if self.transaction == TransactionState::Open {
            self.transaction = TransactionState::Failed;
        }
    }

    /// Handle a generic connection stream (TCP or TLS)
    pub async fn handle_connection<S>(&mut self, stream: S) -> ProtocolResult<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        self.handle_connection_with_buffer(stream, BytesMut::new())
            .await
    }

    /// Handle a connection whose first bytes have already been read.
    ///
    /// TLS negotiation happens before this point and has to read the client's
    /// first 8 bytes to know what was asked for. When those turn out to belong
    /// to the startup message instead, they are passed back in here so the
    /// message can be parsed whole.
    pub async fn handle_connection_with_buffer<S>(
        &mut self,
        mut stream: S,
        prefix: BytesMut,
    ) -> ProtocolResult<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        info!("New PostgreSQL client connection");

        let mut read_buf = BytesMut::with_capacity(8192);
        read_buf.extend_from_slice(&prefix);
        let mut write_buf = BytesMut::with_capacity(8192);

        // The prefix may already hold a complete startup message.
        if !read_buf.is_empty() {
            match self
                .process_pending_messages(&mut stream, &mut read_buf, &mut write_buf)
                .await?
            {
                ConnectionLoopResult::Continue => {}
                ConnectionLoopResult::ClientDisconnected
                | ConnectionLoopResult::ClientTerminated => return Ok(()),
            }
        }

        loop {
            match self
                .read_and_process_data(&mut stream, &mut read_buf, &mut write_buf)
                .await?
            {
                ConnectionLoopResult::Continue => continue,
                ConnectionLoopResult::ClientDisconnected => {
                    info!("Client disconnected");
                    break;
                }
                ConnectionLoopResult::ClientTerminated => {
                    info!("Client terminated connection");
                    self.notifications
                        .disconnect(self.session_notifications.id)
                        .await;
                    return Ok(());
                }
            }
        }

        // A session that has gone must not stay in the registry.
        self.notifications
            .disconnect(self.session_notifications.id)
            .await;

        Ok(())
    }

    /// Read and process data from the client
    async fn read_and_process_data<S>(
        &mut self,
        stream: &mut S,
        read_buf: &mut BytesMut,
        write_buf: &mut BytesMut,
    ) -> ProtocolResult<ConnectionLoopResult>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        // Wait for either the client to say something or a notification to
        // arrive. Reading alone would hold a notification until the client
        // happened to send a message, which for an idle listener is never —
        // and an idle listener is the whole point of LISTEN.
        let n = loop {
            // A replication stream, once started, is the same idle problem as
            // a listener: changes have to reach the standby without waiting
            // for it to say something.
            let streaming = self.replication_stream.is_some();
            tokio::select! {
                read = stream.read_buf(read_buf) => break read?,
                Some(notification) = self.session_notifications.receiver.recv() => {
                    BackendMessage::NotificationResponse {
                        process_id: notification.process_id,
                        channel: notification.channel,
                        payload: notification.payload,
                    }
                    .encode(write_buf);
                    self.flush_write_buffer(stream, write_buf).await?;
                }
                change = async {
                    match self.replication_stream.as_mut() {
                        Some(stream) => stream.recv().await.ok(),
                        None => None,
                    }
                }, if streaming => {
                    if let Some(change) = change {
                        self.send_change(&change, write_buf);
                        self.flush_write_buffer(stream, write_buf).await?;
                    }
                }
            }
        };

        if n == 0 {
            return Ok(ConnectionLoopResult::ClientDisconnected);
        }

        self.process_pending_messages(stream, read_buf, write_buf)
            .await
    }

    /// Process all pending messages in the read buffer
    async fn process_pending_messages<S>(
        &mut self,
        stream: &mut S,
        read_buf: &mut BytesMut,
        write_buf: &mut BytesMut,
    ) -> ProtocolResult<ConnectionLoopResult>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        while let Some(msg) = FrontendMessage::parse_as(read_buf, self.password_message_kind())? {
            debug!("Received message: {:?}", msg);

            match self.process_single_message(msg, write_buf).await {
                MessageResult::Continue => {}
                MessageResult::Terminate => {
                    return Ok(ConnectionLoopResult::ClientTerminated);
                }
                MessageResult::Error(e) => {
                    error!("Error handling message: {}", e);
                    self.send_error_for(write_buf, &e);
                    // The protocol requires a ReadyForQuery after an error
                    // before the client may send anything else. Without it the
                    // client waits for a message that never comes and the
                    // session appears to have died — one bad statement took
                    // the whole connection down.
                    BackendMessage::ReadyForQuery {
                        status: self.transaction_status(),
                    }
                    .encode(write_buf);
                }
            }

            self.flush_write_buffer(stream, write_buf).await?;
        }

        Ok(ConnectionLoopResult::Continue)
    }

    /// Process a single message
    async fn process_single_message(
        &mut self,
        msg: FrontendMessage,
        write_buf: &mut BytesMut,
    ) -> MessageResult {
        match self.handle_message(msg, write_buf).await {
            Ok(should_continue) => {
                if should_continue {
                    MessageResult::Continue
                } else {
                    MessageResult::Terminate
                }
            }
            Err(e) => MessageResult::Error(e),
        }
    }

    /// Flush the write buffer to the stream
    async fn flush_write_buffer<S>(
        &mut self,
        stream: &mut S,
        write_buf: &mut BytesMut,
    ) -> ProtocolResult<()>
    where
        S: tokio::io::AsyncWrite + Unpin,
    {
        if !write_buf.is_empty() {
            stream.write_all(write_buf).await?;
            write_buf.clear();
        }
        Ok(())
    }

    /// Handle a single frontend message
    async fn handle_message(
        &mut self,
        msg: FrontendMessage,
        buf: &mut BytesMut,
    ) -> ProtocolResult<bool> {
        // Everything between a failure and the client's `Sync` is discarded,
        // which is what makes a pipeline stop at its first error rather than
        // running the rest of it.
        // Only the extended protocol enters the skipping state, and only its
        // own messages are discarded by it. A copy stream is exempt as well:
        // its `CopyData`/`CopyDone` are what end the stream, and discarding
        // them leaves both sides waiting forever.
        self.handling_extended = matches!(
            msg,
            FrontendMessage::Parse { .. }
                | FrontendMessage::Bind { .. }
                | FrontendMessage::Execute { .. }
                | FrontendMessage::Describe { .. }
                | FrontendMessage::Close { .. }
        );
        if self.skip_until_sync && self.copy_in.is_none() && self.handling_extended {
            return Ok(true);
        }

        match msg {
            FrontendMessage::Startup {
                protocol_version,
                parameters,
            } => {
                self.handle_startup(protocol_version, parameters, buf)
                    .await?;
            }
            FrontendMessage::Password { password } => {
                self.handle_password(&password, buf).await?;
            }
            FrontendMessage::Query { query } => {
                self.handle_query(&query, buf).await?;
            }
            FrontendMessage::Parse {
                statement_name,
                query,
                param_types,
            } => {
                self.handle_parse(&statement_name, &query, param_types, buf)
                    .await?;
            }
            FrontendMessage::Bind {
                portal,
                statement,
                param_formats,
                params,
                result_formats,
            } => {
                self.handle_bind(
                    &portal,
                    &statement,
                    param_formats,
                    params,
                    result_formats,
                    buf,
                )?;
            }
            FrontendMessage::Execute { portal, max_rows } => {
                self.handle_execute(&portal, max_rows, buf).await?;
            }
            FrontendMessage::Describe { target, name } => {
                self.handle_describe(target, &name, buf).await?;
            }
            FrontendMessage::Close { target, name } => {
                self.handle_close(target, &name, buf)?;
            }
            FrontendMessage::Sync => {
                // While a copy-in stream is open the backend ignores Sync: the
                // client sends one straight after Execute and is not expecting
                // a reply until CopyDone. Answering it here put a
                // ReadyForQuery into the middle of the stream, which is the
                // "unexpected message from server" the client then reported.
                if self.copy_in.is_none() {
                    self.skip_until_sync = false;
                    self.deliver_pending_notifications(buf);
                    BackendMessage::ReadyForQuery {
                        status: self.transaction_status(),
                    }
                    .encode(buf);
                }
            }
            FrontendMessage::Flush => {
                // Nothing to do, data is flushed after each message
            }
            FrontendMessage::Terminate => {
                super::query_engine::forget_cancellable(self.process_id);
                return Ok(false);
            }
            FrontendMessage::CancelRequest {
                process_id,
                secret_key,
            } => {
                // The connection carrying a cancel request is not a session:
                // it sends nothing back and closes, which is what the protocol
                // specifies and what stops it being used to probe for keys.
                super::query_engine::request_cancel(process_id, &secret_key);
                return Ok(false);
            }
            FrontendMessage::SSLRequest => {
                self.handle_ssl_request(buf).await?;
            }
            FrontendMessage::SASLInitialResponse { mechanism, data } => {
                self.handle_sasl_initial_response(&mechanism, data, buf)
                    .await?;
            }
            FrontendMessage::GSSResponse { data } => {
                self.handle_gss_response(&data, buf).await?;
            }
            FrontendMessage::SASLResponse { data } => {
                self.handle_sasl_response(data, buf).await?;
            }
            FrontendMessage::FunctionCall {
                oid,
                args,
                arg_formats,
                result_format,
            } => {
                self.handle_function_call(oid, &args, &arg_formats, result_format, buf)
                    .await?;
            }
            FrontendMessage::CopyData { data } => {
                // On a replication stream a CopyData is the standby telling us
                // how far it has written, flushed and applied.
                if self.replication_stream.is_some() {
                    self.handle_standby_status(&data, buf).await;
                } else {
                    self.handle_copy_data(&data, buf).await?;
                }
            }
            FrontendMessage::CopyDone => {
                self.handle_copy_done(buf).await?;
            }
            FrontendMessage::CopyFail { message } => {
                // The client is abandoning the load; nothing already applied is
                // undone, matching a non-transactional COPY.
                let simple = self.copy_in.take().is_some_and(|s| s.simple_protocol);
                self.send_error(buf, &format!("COPY from stdin failed: {message}"));
                self.finish_copy_statement(simple, buf);
            }
        }

        Ok(true)
    }

    /// Handle startup message
    async fn handle_startup(
        &mut self,
        protocol_version: i32,
        parameters: HashMap<String, String>,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        info!(
            "Startup: protocol_version={}, parameters={:?}",
            protocol_version, parameters
        );

        // PostgreSQL 18 Protocol Version Negotiation
        // Protocol version format: major * 65536 + minor (e.g., 3.0 = 196608, 3.2 = 196610)
        let major = protocol_version >> 16;
        let minor = protocol_version & 0xFFFF;

        // We support protocol 3.0 (fully) and 3.2 (partially - message types defined)
        // If client requests protocol > 3.0, we negotiate down to 3.0
        const SUPPORTED_MAJOR: i32 = 3;
        const SUPPORTED_MINOR: i32 = 0;

        // Check for unrecognized protocol options (those starting with _pq_.)
        let unrecognized_options: Vec<String> = parameters
            .keys()
            .filter(|k| k.starts_with("_pq_."))
            .cloned()
            .collect();

        // Send NegotiateProtocolVersion if needed (PG18 protocol 3.2 feature)
        if major != SUPPORTED_MAJOR || minor > SUPPORTED_MINOR || !unrecognized_options.is_empty() {
            if major == SUPPORTED_MAJOR && minor > SUPPORTED_MINOR {
                // Client requested a newer minor version, negotiate to our supported version
                info!(
                    "Protocol negotiation: client requested {}.{}, negotiating to {}.{}",
                    major, minor, SUPPORTED_MAJOR, SUPPORTED_MINOR
                );
            }
            if !unrecognized_options.is_empty() {
                info!("Unrecognized protocol options: {:?}", unrecognized_options);
            }
            BackendMessage::NegotiateProtocolVersion {
                newest_minor_version: SUPPORTED_MINOR,
                unrecognized_options,
            }
            .encode(buf);
        }

        // `replication=true|database` selects the walsender protocol.
        self.replication = parameters
            .get("replication")
            .is_some_and(|value| !value.eq_ignore_ascii_case("false"));

        self.username = parameters.get("user").cloned();
        self.database = parameters.get("database").cloned();
        self.parameters = parameters;
        self.state = ConnectionState::Authenticating;

        // Auto-register user for SCRAM testing if needed
        if matches!(self.auth_manager.auth_method(), AuthMethod::ScramSha256) {
            if let Some(user) = &self.username {
                if self
                    .auth_manager
                    .user_store()
                    .get_user(user)
                    .await
                    .is_none()
                {
                    // Auto-create user with password same as username for testing
                    self.auth_manager
                        .user_store()
                        .add_user(user.clone(), user.clone(), &AuthMethod::ScramSha256)
                        .await;
                }
            }
        }

        let response = self.auth_manager.get_initial_auth_response();
        if matches!(response, AuthenticationResponse::GSS) && !self.begin_gss(buf) {
            return Ok(());
        }
        BackendMessage::Authentication(response.clone()).encode(buf);

        if let AuthenticationResponse::Ok = response {
            self.finish_authentication(buf);
        }

        Ok(())
    }

    /// How a `'p'` message is to be read on this connection.
    fn password_message_kind(&self) -> PasswordMessageKind {
        #[cfg(feature = "gssapi")]
        if self.gss.is_some() {
            return PasswordMessageKind::GssToken;
        }
        PasswordMessageKind::Credential
    }

    /// Start a GSSAPI handshake, reporting whether it can proceed.
    ///
    /// Returns `false` — having already written an error — when this build
    /// cannot do GSSAPI at all. Announcing `AuthenticationGSS` from a server
    /// with no mechanism behind it would leave the client waiting on a
    /// handshake that can never answer.
    #[allow(unused_variables)]
    fn begin_gss(&mut self, buf: &mut BytesMut) -> bool {
        #[cfg(feature = "gssapi")]
        {
            self.gss = Some(super::gssapi::Acceptor::new());
            true
        }
        #[cfg(not(feature = "gssapi"))]
        {
            let error = ProtocolError::SqlState {
                code: "0A000",
                message: "GSSAPI authentication is configured but this server was built without it"
                    .to_string(),
            };
            self.send_error_for(buf, &error);
            false
        }
    }

    /// The version this server reports, in `ParameterStatus` and in `SHOW`.
    ///
    /// The number leads because clients parse it: libpq takes the digits
    /// before the first non-numeric character, so anything else has to follow.
    const SERVER_VERSION: &'static str = "14.0 (Orbit-RS Protocol Adapter)";

    /// The settings a session starts with, reported by `SHOW`.
    fn advertised_parameters() -> [(&'static str, &'static str); 6] {
        [
            ("server_version", Self::SERVER_VERSION),
            ("server_encoding", "UTF8"),
            ("client_encoding", "UTF8"),
            ("DateStyle", "ISO, MDY"),
            ("integer_datetimes", "on"),
            ("standard_conforming_strings", "on"),
        ]
    }

    /// Finish authentication and unblock connection
    fn finish_authentication(&mut self, buf: &mut BytesMut) {
        // Whatever is advertised here is also what `SHOW` must answer. They
        // came from different places, so the server told a client one thing at
        // connect and another when asked: `SHOW server_version` returned an
        // empty string while `ParameterStatus` carried a version, and a driver
        // reading the empty one cannot tell what it is talking to.
        for (name, value) in Self::advertised_parameters() {
            self.parameters.insert(name.to_string(), value.to_string());
        }

        // Send parameter status
        BackendMessage::ParameterStatus {
            name: "server_version".to_string(),
            value: Self::SERVER_VERSION.to_string(),
        }
        .encode(buf);

        BackendMessage::ParameterStatus {
            name: "server_encoding".to_string(),
            value: "UTF8".to_string(),
        }
        .encode(buf);

        BackendMessage::ParameterStatus {
            name: "client_encoding".to_string(),
            value: "UTF8".to_string(),
        }
        .encode(buf);

        // Send backend key data (PostgreSQL 18: supports variable-length keys).
        // The same key registers the session, so a cancel arriving on another
        // connection can find it.
        self.cancelled = Some(super::query_engine::register_cancellable(
            self.process_id,
            self.secret_key.clone(),
        ));
        BackendMessage::BackendKeyData {
            process_id: self.process_id,
            secret_key: self.secret_key.clone(),
        }
        .encode(buf);

        // Ready for query
        BackendMessage::ReadyForQuery {
            status: TransactionStatus::Idle,
        }
        .encode(buf);

        self.state = ConnectionState::Ready;
    }

    /// Handle password message
    async fn handle_password(&mut self, password: &str, buf: &mut BytesMut) -> ProtocolResult<()> {
        let username = self.username.clone().unwrap_or_default();
        // In a real implementation we would check the password properly
        let valid = self
            .auth_manager
            .verify_password(&username, password, None)
            .await?;

        if valid {
            BackendMessage::Authentication(AuthenticationResponse::Ok).encode(buf);
            self.finish_authentication(buf);
            Ok(())
        } else {
            self.send_error(buf, "Password authentication failed");
            Ok(()) // Don't terminate, just error? Usually terminate on auth fail.
        }
    }

    /// Handle one GSSAPI token from the client.
    ///
    /// Each token is fed to the acceptor, which either asks for another round
    /// or establishes the context and names the principal. The principal is
    /// then checked against the user in the startup packet before the session
    /// is let in — the Kerberos library says *who* the caller is, and nothing
    /// but this check says whether that caller may be this user.
    #[allow(unused_variables)]
    async fn handle_gss_response(&mut self, data: &[u8], buf: &mut BytesMut) -> ProtocolResult<()> {
        #[cfg(not(feature = "gssapi"))]
        {
            let error = ProtocolError::SqlState {
                code: "0A000",
                message: "GSSAPI authentication is not supported by this build".to_string(),
            };
            self.send_error_for(buf, &error);
            Ok(())
        }
        #[cfg(feature = "gssapi")]
        {
            use super::gssapi::{AcceptStep, NameMapping};

            let Some(acceptor) = self.gss.as_mut() else {
                // A token with no handshake open is a client out of step with
                // the protocol; it is not a password, so it must not be tried
                // as one.
                let error = ProtocolError::SqlState {
                    code: "08P01",
                    message: "unexpected GSSAPI token: no authentication is in progress"
                        .to_string(),
                };
                self.send_error_for(buf, &error);
                return Ok(());
            };

            let step = match acceptor.step(data) {
                Ok(step) => step,
                Err(error) => {
                    // The handshake is over either way; keeping the acceptor
                    // would let a client retry against a poisoned context.
                    self.gss = None;
                    // Sent through the error path that keeps the SQLSTATE:
                    // `send_error` re-derives one from the prose, and a
                    // rejected ticket came back as XX000 `internal_error`,
                    // which tells a client to retry something that will never
                    // succeed instead of to fix its credentials.
                    self.send_error_for(buf, &error);
                    return Ok(());
                }
            };

            match step {
                AcceptStep::Continue(token) => {
                    BackendMessage::Authentication(AuthenticationResponse::GSSContinue {
                        data: bytes::Bytes::from(token),
                    })
                    .encode(buf);
                    Ok(())
                }
                AcceptStep::Complete { token, principal } => {
                    // Sent before `Ok`, and before the authorization check:
                    // under mutual authentication this token is what proves
                    // the server's identity, and a client that asked for it
                    // is entitled to it even when the answer is then no.
                    if let Some(token) = token {
                        BackendMessage::Authentication(AuthenticationResponse::GSSContinue {
                            data: bytes::Bytes::from(token),
                        })
                        .encode(buf);
                    }
                    self.gss = None;

                    let requested = self.username.clone().unwrap_or_default();
                    match NameMapping::from_env().authorize(&principal, &requested) {
                        Ok(()) => {
                            info!(%principal, user = %requested, "GSSAPI authentication succeeded");
                            BackendMessage::Authentication(AuthenticationResponse::Ok).encode(buf);
                            self.finish_authentication(buf);
                        }
                        Err(denial) => {
                            warn!(%principal, user = %requested, "GSSAPI authentication refused");
                            let error = ProtocolError::SqlState {
                                // 28000 invalid_authorization_specification,
                                // which is what PostgreSQL reports when a
                                // login is refused.
                                code: "28000",
                                message: denial.message(),
                            };
                            self.send_error_for(buf, &error);
                        }
                    }
                    Ok(())
                }
            }
        }
    }

    /// Handle SASL initial response
    async fn handle_sasl_initial_response(
        &mut self,
        mechanism: &str,
        data: Option<bytes::Bytes>,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        if mechanism != "SCRAM-SHA-256" {
            self.send_error(buf, "Unsupported SASL mechanism");
            return Ok(());
        }

        let username = self.username.clone().unwrap_or_default();
        let user_store = self.auth_manager.user_store();
        let user_creds = user_store.get_user(&username).await;

        if let Some(creds) = user_creds {
            if let (Some(stored_key), Some(server_key), Some(salt), Some(iterations)) = (
                creds.scram_stored_key,
                creds.scram_server_key,
                creds.scram_salt,
                creds.scram_iterations,
            ) {
                // Parse client-first-message to extract client nonce
                let client_first = if let Some(d) = &data {
                    String::from_utf8_lossy(d).to_string()
                } else {
                    "".to_string()
                };

                // Extract 'r=' part (nonce)
                // Format: n,,n=user,r=nonce
                let nonce = client_first
                    .split(',')
                    .find(|p| p.starts_with("r="))
                    .map(|p| p.trim_start_matches("r=").to_string());

                if let Some(client_nonce) = nonce {
                    let mut scram = ScramAuth::new(
                        username.clone(),
                        client_nonce,
                        salt,
                        iterations,
                        stored_key,
                        server_key,
                    );

                    let server_first = scram.process_client_first(&client_first)?;
                    self.scram_auth = Some(scram);

                    BackendMessage::Authentication(AuthenticationResponse::SASLContinue {
                        data: bytes::Bytes::from(server_first),
                    })
                    .encode(buf);
                } else {
                    self.send_error(buf, "Invalid SCRAM client-first-message: missing nonce");
                }
            } else {
                self.send_error(buf, "User not configured for SCRAM");
            }
        } else {
            self.send_error(buf, "Authentication failed");
        }
        Ok(())
    }

    /// Handle SASL response
    async fn handle_sasl_response(
        &mut self,
        data: bytes::Bytes,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        if let Some(scram) = self.scram_auth.take() {
            let client_final = String::from_utf8_lossy(&data).to_string();
            match scram.process_client_final(&client_final) {
                Ok(server_final) => {
                    BackendMessage::Authentication(AuthenticationResponse::SASLFinal {
                        data: bytes::Bytes::from(server_final),
                    })
                    .encode(buf);

                    BackendMessage::Authentication(AuthenticationResponse::Ok).encode(buf);
                    self.finish_authentication(buf);
                }
                Err(e) => {
                    self.send_error(buf, &format!("SCRAM authentication failed: {}", e));
                }
            }
        } else {
            self.send_error(buf, "Protocol error: SASL response without initial step");
        }
        Ok(())
    }

    /// Handle simple query
    async fn handle_query(&mut self, query: &str, buf: &mut BytesMut) -> ProtocolResult<()> {
        info!("Query: {} (database: {:?})", query, self.database);

        self.copy_in_is_simple = true;
        if self.handle_copy_statement(query, buf, true).await.is_some() {
            return Ok(());
        }

        self.snapshot_before_write(query).await;

        if let Some(tag) = self.handle_notification_statement(query).await {
            BackendMessage::CommandComplete { tag }.encode(buf);
            self.deliver_pending_notifications(buf);
            BackendMessage::ReadyForQuery {
                status: self.transaction_status(),
            }
            .encode(buf);
            return Ok(());
        }

        if query.trim().is_empty() {
            BackendMessage::EmptyQueryResponse.encode(buf);
            BackendMessage::ReadyForQuery {
                status: self.transaction_status(),
            }
            .encode(buf);
            return Ok(());
        }

        // A replication connection speaks its own commands.
        if self.replication {
            if let Some(handled) = self.handle_replication_command(query, buf).await {
                return handled;
            }
        }

        // Set the current database context before executing the query
        if let Some(ref db) = self.database {
            self.query_engine.set_current_database(db).await;
        }

        // PostgreSQL rejects everything but a rollback once a statement in the
        // block has failed. Answering them instead let a client believe work
        // done after the failure was part of the committed transaction.
        if self.transaction == TransactionState::Failed && !Self::ends_transaction(query) {
            // `25P02` is `in_failed_sql_transaction`, which is how a driver
            // knows it must roll back rather than retry. Reported as `XX000`
            // it was indistinguishable from the backend falling over.
            self.send_error_for(
                buf,
                &ProtocolError::SqlState {
                    code: "25P02",
                    message: "current transaction is aborted, commands ignored until end of \
                              transaction block"
                        .to_string(),
                },
            );
            BackendMessage::ReadyForQuery {
                status: self.transaction_status(),
            }
            .encode(buf);
            return Ok(());
        }

        // `SET` and `SHOW` share one store, on the session. Routing them
        // separately meant `SHOW` could not see what `SET` had recorded — and
        // `SHOW` did not parse at all.
        if let Some(result) = self.handle_session_parameter(query) {
            match result {
                Ok(Some(shown)) => self.send_query_result(&shown, buf),
                Ok(None) => BackendMessage::CommandComplete {
                    tag: "SET".to_string(),
                }
                .encode(buf),
                Err(message) => {
                    self.send_error(buf, &message);
                    self.note_failure();
                }
            }
            BackendMessage::ReadyForQuery {
                status: self.transaction_status(),
            }
            .encode(buf);
            return Ok(());
        }

        // `SET CONSTRAINTS ALL IMMEDIATE` validates the deferrable keys now
        // rather than waiting for COMMIT, which is what it is for: finding out
        // whether the block will commit before committing it.
        // `BEGIN ISOLATION LEVEL ...` and `SET TRANSACTION ISOLATION LEVEL ...`
        // choose between reading through a snapshot and seeing each commit.
        let upper_query = query.to_uppercase();
        if upper_query.contains("ISOLATION LEVEL") {
            self.serializable = upper_query.contains("SERIALIZABLE");
            self.snapshot_isolation = self.serializable || upper_query.contains("REPEATABLE READ");
        }

        if query
            .trim_start()
            .get(..15)
            .is_some_and(|head| head.eq_ignore_ascii_case("SET CONSTRAINTS"))
        {
            // `DEFERRED` puts the checks back to COMMIT; `IMMEDIATE` runs them
            // now and keeps running them per statement for the rest of the
            // block, which is the difference the two modes are for.
            self.constraints_immediate = query.to_uppercase().contains("IMMEDIATE");
            if self.constraints_immediate {
                if let Err(e) = self
                    .query_engine
                    .check_deferred_constraints(&self.tables_written())
                    .await
                {
                    self.send_error_for(buf, &e);
                    self.note_failure();
                    BackendMessage::ReadyForQuery {
                        status: self.transaction_status(),
                    }
                    .encode(buf);
                    return Ok(());
                }
            }
            BackendMessage::CommandComplete {
                tag: "SET CONSTRAINTS".to_string(),
            }
            .encode(buf);
            BackendMessage::ReadyForQuery {
                status: self.transaction_status(),
            }
            .encode(buf);
            return Ok(());
        }

        if let Some(handled) = self.handle_savepoint(query).await {
            match handled {
                Ok(tag) => BackendMessage::CommandComplete { tag }.encode(buf),
                Err(message) => {
                    self.send_error(buf, &message);
                    self.note_failure();
                }
            }
            BackendMessage::ReadyForQuery {
                status: self.transaction_status(),
            }
            .encode(buf);
            return Ok(());
        }

        // Statements run inside the session's transaction so the rows they
        // write carry its stamp and stay private until it ends.
        let run = async {
            match self.transaction_id.clone() {
                Some(context) => {
                    super::query_engine::within_transaction(
                        context,
                        self.query_engine.execute_multiple_queries(query),
                    )
                    .await
                }
                None => self.query_engine.execute_multiple_queries(query).await,
            }
        };
        // The session's cancel flag travels with the statement, so the engine
        // can check it between the statements of one message.
        let executed = match self.cancelled.clone() {
            Some(flag) => super::query_engine::with_cancel(flag, run).await,
            None => run.await,
        };

        match executed {
            Ok(results) => {
                for result in results {
                    self.send_query_result(&result, buf);
                }
                // With IMMEDIATE in force, a deferrable key is checked after
                // every statement rather than only at COMMIT. Transaction
                // control is exempt: failing the check on ROLLBACK would leave
                // the session unable to leave the block at all.
                if self.constraints_immediate
                    && self.transaction == TransactionState::Open
                    && !Self::ends_transaction(query)
                {
                    if let Err(e) = self
                        .query_engine
                        .check_deferred_constraints(&self.tables_written())
                        .await
                    {
                        self.send_error_for(buf, &e);
                        self.note_failure();
                        BackendMessage::ReadyForQuery {
                            status: self.transaction_status(),
                        }
                        .encode(buf);
                        return Ok(());
                    }
                }
                self.apply_transaction_control(query, buf).await;
                self.note_statement(query);
                self.deliver_pending_notifications(buf);
                BackendMessage::ReadyForQuery {
                    status: self.transaction_status(),
                }
                .encode(buf);
            }
            Err(e) => {
                self.send_error_for(buf, &e);
                self.note_failure();
                BackendMessage::ReadyForQuery {
                    status: self.transaction_status(),
                }
                .encode(buf);
            }
        }

        Ok(())
    }

    /// Handle parse message (prepared statement)
    async fn handle_parse(
        &mut self,
        statement_name: &str,
        query: &str,
        param_types: Vec<i32>,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Parse: name={}, query={}", statement_name, query);

        self.prepared_statements
            .insert(statement_name.to_string(), query.to_string());

        // A client may send fewer type OIDs than the statement has
        // placeholders, or send 0 for "you decide". Those were all filled in
        // as text, so `WHERE id = $1` compared an integer column against
        // `'2'` and matched nothing — the exact failure `bind_parameters`
        // says it guards against, arriving from the other side. The engine
        // already works the type out from the column each placeholder is
        // compared against; it just was not asked.
        let placeholders = Self::count_placeholders(query);
        let mut param_types = param_types;
        param_types.resize(param_types.len().max(placeholders), 0);

        if param_types.contains(&0) {
            let inferred = self
                .query_engine
                .describe_parameters(query)
                .await
                .unwrap_or_default();
            for (position, oid) in param_types.iter_mut().enumerate() {
                if *oid == 0 {
                    *oid = inferred
                        .get(position)
                        .copied()
                        .filter(|inferred| *inferred != 0)
                        .unwrap_or(super::messages::type_oids::TEXT);
                }
            }
        }
        self.statement_param_types
            .insert(statement_name.to_string(), param_types);

        BackendMessage::ParseComplete.encode(buf);
        Ok(())
    }

    /// Whether a value of this type is written into SQL without quotes.
    fn is_unquoted_literal_type(type_oid: i32) -> bool {
        matches!(
            type_oid,
            type_oids::INT2
                | type_oids::INT4
                | type_oids::INT8
                | type_oids::FLOAT4
                | type_oids::FLOAT8
                // An exact decimal is a number too. Quoted, `amt = $1`
                // compared a `NUMERIC` column against a string and matched
                // nothing.
                | type_oids::NUMERIC
                | type_oids::BOOL
        )
    }

    /// Whether `text` is safe to splice in unquoted.
    ///
    /// Belt and braces: the type says the value should be bare, but the bytes
    /// come from the client. Anything that is not plainly a number or a boolean
    /// is quoted instead, so a hostile value cannot become syntax.
    fn is_safe_bare_literal(text: &str) -> bool {
        if matches!(
            text.to_ascii_lowercase().as_str(),
            "true" | "false" | "t" | "f"
        ) {
            return true;
        }

        !text.is_empty()
            && text.parse::<f64>().is_ok()
            && text
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))
    }

    /// Highest `$n` placeholder appearing in `query`.
    ///
    /// Counts distinct positions rather than occurrences: `$1 AND $1` is one
    /// parameter. Text inside single-quoted literals is skipped so a `$1` in a
    /// string is not mistaken for a placeholder.
    fn count_placeholders(query: &str) -> usize {
        let bytes = query.as_bytes();
        let mut highest = 0usize;
        let mut index = 0usize;
        let mut in_literal = false;

        while index < bytes.len() {
            match bytes[index] {
                b'\'' => {
                    in_literal = !in_literal;
                    index += 1;
                }
                b'$' if !in_literal => {
                    let start = index + 1;
                    let end = start
                        + bytes[start..]
                            .iter()
                            .take_while(|b| b.is_ascii_digit())
                            .count();
                    if end > start {
                        if let Ok(n) = query[start..end].parse::<usize>() {
                            highest = highest.max(n);
                        }
                    }
                    index = end.max(index + 1);
                }
                _ => index += 1,
            }
        }

        highest
    }

    /// Handle bind message
    fn handle_bind(
        &mut self,
        portal: &str,
        statement: &str,
        param_formats: Vec<i16>,
        params: Vec<Option<bytes::Bytes>>,
        result_formats: Vec<i16>,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Bind: portal={}, statement={}", portal, statement);
        self.portal_result_formats
            .insert(portal.to_string(), result_formats);

        // Format codes are per-parameter, or a single code covering all of them,
        // or empty meaning all text.
        let is_binary = |index: usize| match param_formats.len() {
            0 => false,
            1 => param_formats[0] == 1,
            _ => param_formats.get(index).is_some_and(|f| *f == 1),
        };

        let declared = self
            .statement_param_types
            .get(statement)
            .cloned()
            .unwrap_or_default();

        // Parameters are stored as text, because that is what substitution into
        // the statement needs. Binary values are decoded here, where the
        // declared type says how to read the bytes.
        let mut decoded = Vec::with_capacity(params.len());
        for (index, value) in params.into_iter().enumerate() {
            let Some(raw) = value else {
                decoded.push(None);
                continue;
            };

            if !is_binary(index) {
                decoded.push(Some(raw));
                continue;
            }

            let type_oid = declared.get(index).copied().unwrap_or(type_oids::TEXT);
            match Self::decode_binary_parameter(&raw, type_oid) {
                Ok(text) => decoded.push(Some(bytes::Bytes::from(text))),
                Err(e) => {
                    self.send_error(buf, &format!("parameter ${}: {e}", index + 1));
                    return Ok(());
                }
            }
        }

        // Re-binding a portal restarts it, so any partially delivered result
        // from a previous execution is discarded.
        self.portal_rows.remove(portal);
        self.portals
            .insert(portal.to_string(), (statement.to_string(), decoded));

        BackendMessage::BindComplete.encode(buf);
        Ok(())
    }

    /// Render a binary-format parameter as the text the engine works with.
    ///
    /// PostgreSQL's binary encodings are big-endian and fixed width for the
    /// scalar types. A type this does not know is rejected rather than guessed
    /// at, because misreading the bytes would substitute a wrong value into the
    /// statement without any error.
    fn decode_binary_parameter(raw: &[u8], type_oid: i32) -> Result<String, String> {
        fn fixed<const N: usize>(raw: &[u8], type_name: &str) -> Result<[u8; N], String> {
            raw.try_into()
                .map_err(|_| format!("expected {N} bytes for {type_name}, got {}", raw.len()))
        }

        match type_oid {
            type_oids::BOOL => match raw {
                [0] => Ok("false".to_string()),
                [1] => Ok("true".to_string()),
                _ => Err("expected a single 0 or 1 byte for bool".to_string()),
            },
            type_oids::INT2 => Ok(i16::from_be_bytes(fixed::<2>(raw, "int2")?).to_string()),
            type_oids::INT4 => Ok(i32::from_be_bytes(fixed::<4>(raw, "int4")?).to_string()),
            type_oids::INT8 => Ok(i64::from_be_bytes(fixed::<8>(raw, "int8")?).to_string()),
            type_oids::FLOAT4 => Ok(f32::from_be_bytes(fixed::<4>(raw, "float4")?).to_string()),
            type_oids::FLOAT8 => Ok(f64::from_be_bytes(fixed::<8>(raw, "float8")?).to_string()),
            // These are already UTF-8 on the wire in both formats.
            type_oids::TEXT | type_oids::VARCHAR | type_oids::JSON | type_oids::UUID => {
                std::str::from_utf8(raw)
                    .map(str::to_string)
                    .map_err(|_| "value is not valid UTF-8".to_string())
            }
            other => Err(format!(
                "binary format is not supported for type OID {other}; send this parameter in \
                 text format"
            )),
        }
    }

    /// Substitute bound parameters into `$n` placeholders.
    ///
    /// Values are quoted as SQL literals, so a parameter containing a quote or a
    /// semicolon becomes data rather than syntax. Placeholders inside string
    /// literals are left alone.
    ///
    /// # Errors
    /// Returns an error when the statement references a parameter that was not
    /// bound; executing with a literal `$n` left in place would silently return
    /// the wrong rows.
    fn bind_parameters(
        query: &str,
        params: &[Option<bytes::Bytes>],
        param_types: &[i32],
    ) -> ProtocolResult<String> {
        let bytes = query.as_bytes();
        let mut out = String::with_capacity(query.len());
        let mut index = 0usize;
        let mut in_literal = false;

        while index < bytes.len() {
            let ch = bytes[index];

            if ch == b'\'' {
                in_literal = !in_literal;
                out.push('\'');
                index += 1;
                continue;
            }

            if ch != b'$' || in_literal {
                out.push(bytes[index] as char);
                index += 1;
                continue;
            }

            let start = index + 1;
            let end = start
                + bytes[start..]
                    .iter()
                    .take_while(|b| b.is_ascii_digit())
                    .count();

            if end == start {
                out.push('$');
                index += 1;
                continue;
            }

            let position: usize = query[start..end].parse().map_err(|_| {
                ProtocolError::PostgresError(format!(
                    "invalid parameter placeholder ${}",
                    &query[start..end]
                ))
            })?;

            let value = params.get(position.wrapping_sub(1)).ok_or_else(|| {
                ProtocolError::PostgresError(format!(
                    "statement references ${position} but only {} parameter(s) were bound",
                    params.len()
                ))
            })?;

            match value {
                None => out.push_str("NULL"),
                Some(raw) => {
                    let text = std::str::from_utf8(raw).map_err(|_| {
                        ProtocolError::PostgresError(format!(
                            "parameter ${position} is not valid UTF-8 text"
                        ))
                    })?;

                    let type_oid = param_types
                        .get(position - 1)
                        .copied()
                        .unwrap_or(type_oids::TEXT);

                    if Self::is_unquoted_literal_type(type_oid) && Self::is_safe_bare_literal(text)
                    {
                        // A numeric or boolean parameter has to be emitted
                        // bare: quoting it turns `id = $1` into `id = '1'`,
                        // which compares an integer column against a string
                        // and silently matches nothing.
                        out.push_str(text);
                    } else {
                        out.push('\'');
                        // Doubling is how a single quote is escaped in a SQL literal.
                        out.push_str(&text.replace('\'', "''"));
                        out.push('\'');
                    }
                }
            }

            index = end;
        }

        Ok(out)
    }

    /// Handle execute message
    async fn handle_execute(
        &mut self,
        portal: &str,
        max_rows: i32,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Execute: portal={}, max_rows={}", portal, max_rows);

        // A portal already drained by a previous Execute returns nothing more.
        if let Some(remaining) = self.portal_rows.get(portal) {
            let already_sent = remaining.sent;
            let rows = remaining.rows.clone();
            let columns = remaining.columns.clone();
            self.send_portal_page(portal, &columns, &rows, already_sent, max_rows, buf)
                .await;
            return Ok(());
        }

        let (statement_name, params) = self
            .portals
            .get(portal)
            .ok_or_else(|| ProtocolError::PostgresError(format!("Portal not found: {portal}")))?;

        let query = self
            .prepared_statements
            .get(statement_name)
            .ok_or_else(|| {
                ProtocolError::PostgresError(format!("Statement not found: {statement_name}"))
            })?;

        let param_types = self
            .statement_param_types
            .get(statement_name)
            .cloned()
            .unwrap_or_default();

        let bound = match Self::bind_parameters(query, params, &param_types) {
            Ok(bound) => bound,
            Err(e) => {
                self.send_error_for(buf, &e);
                return Ok(());
            }
        };

        // An empty statement is not an error. PostgreSQL answers
        // `EmptyQueryResponse`, which is how a client tells "nothing to run"
        // from "your statement was rejected"; the simple-query path already
        // did this and the extended one reported a parse failure instead.
        if bound.trim().is_empty() {
            BackendMessage::EmptyQueryResponse.encode(buf);
            return Ok(());
        }

        self.copy_in_is_simple = false;
        if self
            .handle_copy_statement(&bound, buf, false)
            .await
            .is_some()
        {
            return Ok(());
        }

        let bound_for_state = bound.clone();
        let portal_name = portal.to_string();
        // The extended protocol runs inside the session's transaction too.
        let executed = match self.transaction_id.clone() {
            Some(context) => {
                super::query_engine::within_transaction(
                    context,
                    self.query_engine.execute_query(&bound),
                )
                .await
            }
            None => self.query_engine.execute_query(&bound).await,
        };

        match executed {
            Ok(QueryResult::Select { columns, rows })
            | Ok(QueryResult::Merge { columns, rows, .. }) => {
                self.note_statement(&bound_for_state);
                // Remembered so a later Execute on the same portal continues
                // where this one stopped, which is how every driver implements
                // a cursor with a fetch size.
                self.portal_rows.insert(
                    portal_name.clone(),
                    PortalRows {
                        columns: columns.clone(),
                        rows: rows.clone(),
                        sent: 0,
                    },
                );
                self.send_portal_page(&portal_name, &columns, &rows, 0, max_rows, buf)
                    .await;
            }
            Ok(result) => {
                // Extended protocol: the row description was already sent in
                // response to Describe, and repeating it here is a protocol
                // violation. Only the rows and the command tag belong on Execute.
                self.send_query_result_without_description(&result, buf);
                self.note_statement(&bound_for_state);
            }
            Err(e) => {
                self.send_error_for(buf, &e);
                self.note_failure();
            }
        }

        Ok(())
    }

    /// Handle describe message
    /// Handle a Describe message.
    ///
    /// The extended query protocol requires:
    ///
    /// * `Describe(Statement)` → `ParameterDescription`, then `RowDescription`
    ///   or `NoData`;
    /// * `Describe(Portal)` → `RowDescription` or `NoData`.
    ///
    /// Sending only `NoData` — as this did — leaves every conforming driver
    /// reading a `ParameterDescription` it never receives, which is why
    /// `prepare()` failed with "unexpected message from server".
    async fn handle_describe(
        &mut self,
        target: super::messages::DescribeTarget,
        name: &str,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        use super::messages::DescribeTarget;

        debug!("Describe: target={:?}, name={}", target, name);

        let sql = match target {
            DescribeTarget::Statement => self.prepared_statements.get(name).cloned(),
            DescribeTarget::Portal => self
                .portals
                .get(name)
                .and_then(|(statement, _)| self.prepared_statements.get(statement).cloned()),
        };

        let Some(sql) = sql else {
            self.send_error(
                buf,
                &match target {
                    DescribeTarget::Statement => format!("Statement not found: {name}"),
                    DescribeTarget::Portal => format!("Portal not found: {name}"),
                },
            );
            return Ok(());
        };

        // Parameter types belong only to a statement description; a portal's
        // parameters are already bound.
        if matches!(target, DescribeTarget::Statement) {
            // A type the client declared in Parse is authoritative — it says how
            // that client will serialise the value. Where it declared nothing,
            // the type is inferred from how the parameter is used, so callers
            // are not forced to stringify every value.
            let declared = self
                .statement_param_types
                .get(name)
                .cloned()
                .unwrap_or_default();
            let inferred = self.query_engine.describe_parameters(&sql).await?;

            let param_types: Vec<i32> = (0..declared.len().max(inferred.len()))
                .map(|i| match declared.get(i).copied() {
                    Some(oid) if oid != type_oids::TEXT => oid,
                    _ => inferred.get(i).copied().unwrap_or(type_oids::TEXT),
                })
                .collect();

            // Remember what was advertised: Bind decodes binary values with it.
            self.statement_param_types
                .insert(name.to_string(), param_types.clone());

            BackendMessage::ParameterDescription { param_types }.encode(buf);
        }

        let description = self.query_engine.describe_statement(&sql).await?;

        if matches!(target, DescribeTarget::Statement) || description.returns_rows() {
            // Remembered for Execute, which must encode each value in the
            // format the client asked for and therefore needs its type.
            let statement_name = match target {
                DescribeTarget::Statement => name.to_string(),
                DescribeTarget::Portal => self
                    .portals
                    .get(name)
                    .map(|(statement, _)| statement.clone())
                    .unwrap_or_default(),
            };
            self.statement_columns.insert(
                statement_name,
                description.columns.iter().map(|c| c.type_oid).collect(),
            );
        }

        if description.returns_rows() {
            let fields = description
                .columns
                .iter()
                .map(|column| FieldDescription {
                    name: column.name.clone(),
                    table_oid: 0,
                    column_id: 0,
                    type_oid: column.type_oid,
                    type_size: -1,
                    type_modifier: -1,
                    format: 0,
                })
                .collect();
            BackendMessage::RowDescription { fields }.encode(buf);
        } else {
            BackendMessage::NoData.encode(buf);
        }

        Ok(())
    }

    /// Handle close message
    fn handle_close(
        &mut self,
        target: super::messages::CloseTarget,
        name: &str,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        debug!("Close: target={:?}, name={}", target, name);

        match target {
            super::messages::CloseTarget::Statement => {
                self.prepared_statements.remove(name);
            }
            super::messages::CloseTarget::Portal => {
                self.portals.remove(name);
                self.portal_rows.remove(name);
                self.portal_result_formats.remove(name);
            }
        }

        BackendMessage::CloseComplete.encode(buf);
        Ok(())
    }

    /// Send query result
    fn send_query_result(&self, result: &QueryResult, buf: &mut BytesMut) {
        // Simple protocol: the row description precedes the rows.
        if let QueryResult::Select { columns, .. } | QueryResult::Merge { columns, .. } = result {
            // Every value this engine holds is text, and it is sent in text
            // format. Types were previously guessed from the characters of the
            // first row, which labelled a column of '01234' as int4 and made
            // conforming clients decode it as 1234 — losing the leading zero.
            // Advertising text describes what is actually on the wire.
            let fields: Vec<FieldDescription> = columns
                .iter()
                .map(|col| FieldDescription {
                    name: col.clone(),
                    table_oid: 0,
                    column_id: 0,
                    type_oid: type_oids::TEXT,
                    type_size: -1,
                    type_modifier: -1,
                    format: 0,
                })
                .collect();
            BackendMessage::RowDescription { fields }.encode(buf);
        }

        self.send_query_result_without_description(result, buf);
    }

    /// Handle `LISTEN`, `UNLISTEN` and `NOTIFY`, returning the command tag.
    ///
    /// Returns `None` for anything else, which then runs as an ordinary
    /// statement. These are handled here rather than in the SQL engine because
    /// they act on the connection, not on stored data.
    async fn handle_notification_statement(&mut self, query: &str) -> Option<String> {
        let trimmed = query.trim().trim_end_matches(';').trim();
        let (head, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((head, rest)) => (head.to_ascii_uppercase(), rest.trim()),
            None => (trimmed.to_ascii_uppercase(), ""),
        };

        match head.as_str() {
            "LISTEN" if !rest.is_empty() => {
                self.notifications
                    .listen(
                        rest,
                        self.session_notifications.id,
                        self.session_notifications.sender.clone(),
                    )
                    .await;
                Some("LISTEN".to_string())
            }
            "UNLISTEN" => {
                let channel = (rest != "*" && !rest.is_empty()).then_some(rest);
                self.notifications
                    .unlisten(channel, self.session_notifications.id)
                    .await;
                Some("UNLISTEN".to_string())
            }
            "NOTIFY" if !rest.is_empty() => {
                // `NOTIFY channel` or `NOTIFY channel, 'payload'`.
                let (channel, payload) = match rest.split_once(',') {
                    Some((channel, payload)) => (
                        channel.trim(),
                        payload.trim().trim_matches('\'').to_string(),
                    ),
                    None => (rest, String::new()),
                };
                self.notifications
                    .notify(channel, &payload, self.process_id)
                    .await;
                Some("NOTIFY".to_string())
            }
            _ => None,
        }
    }

    /// Write any notifications waiting for this session.
    ///
    /// The protocol allows a NotificationResponse between messages, so they are
    /// flushed at the points the session is already writing.
    fn deliver_pending_notifications(&mut self, buf: &mut BytesMut) {
        while let Ok(notification) = self.session_notifications.receiver.try_recv() {
            BackendMessage::NotificationResponse {
                process_id: notification.process_id,
                channel: notification.channel,
                payload: notification.payload,
            }
            .encode(buf);
        }
    }

    /// Handle a standby status update, and answer a keepalive that asks for one.
    ///
    /// The confirmed position is written to the slot, which is what lets a
    /// replica reconnect and resume where it left off.
    async fn handle_standby_status(&mut self, data: &[u8], buf: &mut BytesMut) {
        match data.first() {
            // `r`: write, flush and apply positions, then a reply flag.
            Some(b'r') if data.len() >= 34 => {
                let flushed = u64::from_be_bytes(data[9..17].try_into().unwrap_or([0; 8]));
                if let Some(slot) = self.replication_slot_name.clone() {
                    if let Err(e) = self
                        .query_engine
                        .confirm_replication_slot(&slot, flushed)
                        .await
                    {
                        tracing::warn!("could not record replica progress: {e}");
                    }
                }
                // The last byte asks for an immediate reply.
                if data[33] == 1 {
                    self.send_keepalive(buf, false);
                }
            }
            // `k`: a keepalive from the other direction.
            Some(b'k') => {}
            _ => {}
        }
    }

    /// Send a keepalive, optionally asking the standby to answer.
    fn send_keepalive(&self, buf: &mut BytesMut, reply_requested: bool) {
        let position = super::query_engine::latest_change_position();
        let mut message = BytesMut::new();
        message.put_u8(b'k');
        message.put_u64(position);
        message.put_i64(0);
        message.put_u8(u8::from(reply_requested));
        BackendMessage::CopyData {
            data: message.freeze(),
        }
        .encode(buf);
    }

    /// Send one change as an `XLogData` message on a replication stream.
    ///
    /// The payload is the change rendered as JSON — an output plugin's job in
    /// PostgreSQL. The header carries the same three positions PostgreSQL
    /// sends, so a standby's bookkeeping has somewhere to start.
    fn send_change(&mut self, change: &super::query_engine::ChangeRecord, buf: &mut BytesMut) {
        let position = change.position;
        let payload = if self.replication_plugin.eq_ignore_ascii_case("pgoutput") {
            self.pgoutput_payload(change)
        } else if change.action == "COMMIT" {
            // The JSON plugin has no transaction framing, so a marker carries
            // nothing a subscriber could use.
            return;
        } else {
            format!(
                "{{\"action\":\"{}\",\"table\":\"{}\",\"xid\":{},\"row\":{}}}",
                change.action, change.table, change.transaction, change.row
            )
            .into_bytes()
        };

        let mut message = BytesMut::new();
        message.put_u8(b'w');
        message.put_u64(position); // start of this record
        message.put_u64(position); // current end of WAL
        message.put_i64(0); // server clock, which this does not track
        message.extend_from_slice(&payload);

        BackendMessage::CopyData {
            data: message.freeze(),
        }
        .encode(buf);
    }

    /// Render a change in the `pgoutput` protocol a real subscriber decodes.
    ///
    /// Each change is a `Begin`, a `Relation` describing the table the first
    /// time it appears, the row message itself, and a `Commit` — the shape
    /// PostgreSQL sends for a single-statement transaction. Values go as text,
    /// which `pgoutput` allows.
    fn pgoutput_payload(&mut self, change: &super::query_engine::ChangeRecord) -> Vec<u8> {
        let row: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_str(&change.row).unwrap_or_default();
        let relation = self.relation_id(&change.table);
        let mut out = BytesMut::new();

        // A `COMMIT` marker closes the pair a block opened.
        if change.action == "COMMIT" {
            out.put_u8(b'C');
            out.put_u8(0);
            out.put_u64(change.position);
            out.put_u64(change.position);
            out.put_i64(0);
            self.replication_open_transaction = None;
            return out.to_vec();
        }

        // Begin once per transaction: a block's statements belong to one.
        let grouped = change.transaction != 0
            && self.replication_open_transaction == Some(change.transaction);
        if !grouped {
            out.put_u8(b'B');
            out.put_u64(change.position);
            out.put_i64(0);
            out.put_i32(change.transaction as i32);
            self.replication_open_transaction = Some(change.transaction);
        }

        // Relation, sent once per table per stream, as the protocol expects.
        if self.announced_relations.insert(change.table.clone()) {
            out.put_u8(b'R');
            out.put_i32(relation);
            out.extend_from_slice(b"public\0");
            out.extend_from_slice(change.table.as_bytes());
            out.put_u8(0);
            out.put_u8(b'd'); // replica identity: default
            out.put_i16(row.len() as i16);
            for name in row.keys() {
                out.put_u8(0); // not part of the key
                out.extend_from_slice(name.as_bytes());
                out.put_u8(0);
                out.put_i32(super::messages::type_oids::TEXT);
                out.put_i32(-1);
            }
        }

        let binary = self.replication_binary;
        let tuple =
            move |out: &mut BytesMut,
                  row: &std::collections::BTreeMap<String, serde_json::Value>| {
                out.put_u8(b'N'); // a new tuple follows
                out.put_i16(row.len() as i16);
                for value in row.values() {
                    match value {
                        serde_json::Value::Null => out.put_u8(b'n'),
                        other => Self::put_replication_value(out, other, binary),
                    }
                }
            };

        match change.action.as_str() {
            "INSERT" => {
                out.put_u8(b'I');
                out.put_i32(relation);
                tuple(&mut out, &row);
            }
            "UPDATE" => {
                out.put_u8(b'U');
                out.put_i32(relation);
                tuple(&mut out, &row);
            }
            "DELETE" => {
                out.put_u8(b'D');
                out.put_i32(relation);
                // The old row identifies what went; `K` is the key tuple.
                out.put_u8(b'K');
                out.put_i16(row.len() as i16);
                let binary = self.replication_binary;
                for value in row.values() {
                    Self::put_replication_value(&mut out, value, binary);
                }
            }
            _ => {}
        }

        // A statement outside a block is its own transaction, so it commits
        // straight away; one inside a block waits for the marker.
        if change.transaction == 0 {
            out.put_u8(b'C');
            out.put_u8(0);
            out.put_u64(change.position);
            out.put_u64(change.position);
            out.put_i64(0);
            self.replication_open_transaction = None;
        }

        out.to_vec()
    }

    /// Write one column value into a `pgoutput` tuple.
    ///
    /// `t` is the text form the protocol defaults to; `b` is the binary form a
    /// subscriber gets when it asks for it, which for a number is the network
    /// byte order PostgreSQL sends rather than its decimal spelling.
    fn put_replication_value(out: &mut BytesMut, value: &serde_json::Value, binary: bool) {
        if binary {
            if let Some(number) = value.as_i64() {
                out.put_u8(b'b');
                out.put_i32(8);
                out.put_i64(number);
                return;
            }
            if let Some(number) = value.as_f64() {
                out.put_u8(b'b');
                out.put_i32(8);
                out.put_f64(number);
                return;
            }
            if let Some(flag) = value.as_bool() {
                out.put_u8(b'b');
                out.put_i32(1);
                out.put_u8(u8::from(flag));
                return;
            }
        }

        let text = value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string());
        out.put_u8(if binary { b'b' } else { b't' });
        out.put_i32(text.len() as i32);
        out.extend_from_slice(text.as_bytes());
    }

    /// A stable id for a table within this stream.
    fn relation_id(&mut self, table: &str) -> i32 {
        let next = self.relation_ids.len() as i32 + 16_384;
        *self.relation_ids.entry(table.to_string()).or_insert(next)
    }

    /// Start a `COPY` statement, or return `None` if this is not one.
    ///
    /// Text format only: the binary format needs per-type encoders this engine
    /// does not have, and accepting it while writing text would corrupt the
    /// stream rather than fail.
    async fn handle_copy_statement(
        &mut self,
        query: &str,
        buf: &mut BytesMut,
        send_ready: bool,
    ) -> Option<()> {
        let trimmed = query.trim().trim_end_matches(';').trim();
        if !trimmed.to_ascii_uppercase().starts_with("COPY ") {
            return None;
        }

        let upper = trimmed.to_ascii_uppercase();
        let binary = upper.contains(" BINARY") || upper.contains("FORMAT BINARY");
        // `WITH CSV` and `FORMAT CSV` were parsed by nothing, so a client that
        // asked for CSV was written tab-separated text and had its CSV input
        // read as one field.
        let csv = !binary && (upper.contains(" CSV") || upper.contains("FORMAT CSV"));

        // `COPY <table> [(cols)] TO STDOUT` / `FROM STDIN`
        let body = trimmed[5..].trim();
        let to_stdout = upper.contains(" TO ");
        let split_at = if to_stdout {
            upper.find(" TO ")
        } else {
            upper.find(" FROM ")
        };
        let Some(split_at) = split_at else {
            self.send_error(buf, "COPY requires TO STDOUT or FROM STDIN");
            self.finish_copy_statement(send_ready, buf);
            return Some(());
        };

        let target = trimmed[5..split_at].trim();
        let (table, columns) = match target.split_once('(') {
            Some((table, cols)) => (
                table.trim().to_string(),
                cols.trim_end_matches(')')
                    .split(',')
                    .map(|c| c.trim().to_string())
                    .collect::<Vec<_>>(),
            ),
            None => (target.to_string(), Vec::new()),
        };
        let _ = body;

        if to_stdout {
            self.copy_table_to_stdout(&table, &columns, binary, csv, buf)
                .await;
            self.finish_copy_statement(send_ready, buf);
        } else {
            self.begin_copy_from_stdin(&table, columns, binary, csv, buf)
                .await;
        }
        Some(())
    }

    /// Close out a COPY statement on the simple query path.
    ///
    /// The extended protocol sends ReadyForQuery in response to Sync instead,
    /// so sending one here too would leave the client a message ahead.
    fn finish_copy_statement(&mut self, send_ready: bool, buf: &mut BytesMut) {
        if send_ready {
            BackendMessage::ReadyForQuery {
                status: self.transaction_status(),
            }
            .encode(buf);
        }
    }

    /// Stream a table to the client as `COPY ... TO STDOUT` text.
    /// Render one CSV field, quoting it only when it needs quoting.
    ///
    /// A field is quoted when it holds a comma, a quote, or a line break;
    /// inside quotes a quote is doubled. That is the shape PostgreSQL writes
    /// and the one a spreadsheet reads back.
    fn csv_field(text: &str) -> String {
        if text.contains([',', '"', '\n', '\r']) {
            return format!("\"{}\"", text.replace('"', "\"\""));
        }
        text.to_string()
    }

    /// Split one CSV line into fields, honouring quotes.
    ///
    /// Returns each field with its quoting removed. A doubled quote inside a
    /// quoted field is one quote.
    fn split_csv_line(line: &str) -> Vec<String> {
        let mut fields = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();

        while let Some(character) = chars.next() {
            match character {
                '"' if in_quotes => {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        current.push('"');
                    } else {
                        in_quotes = false;
                    }
                }
                '"' => in_quotes = true,
                ',' if !in_quotes => fields.push(std::mem::take(&mut current)),
                other => current.push(other),
            }
        }
        fields.push(current);
        fields
    }

    async fn copy_table_to_stdout(
        &mut self,
        table: &str,
        columns: &[String],
        binary: bool,
        csv: bool,
        buf: &mut BytesMut,
    ) {
        let projection = if columns.is_empty() {
            "*".to_string()
        } else {
            columns.join(", ")
        };

        let result = self
            .query_engine
            .execute_query(&format!("SELECT {projection} FROM {table}"))
            .await;

        let (column_count, rows) = match result {
            Ok(QueryResult::Select { columns, rows }) => (columns.len(), rows),
            Ok(_) => (0, Vec::new()),
            Err(e) => {
                self.send_error_for(buf, &e);
                return;
            }
        };

        BackendMessage::CopyOutResponse {
            format: i8::from(binary),
            column_formats: vec![i16::from(binary); column_count],
        }
        .encode(buf);

        if binary {
            // The binary stream opens with a fixed signature, a flags word and
            // an (empty) header extension, and closes with a field count of
            // -1. Each tuple is a field count then length-prefixed values.
            let types = self
                .query_engine
                .describe_statement(&format!("SELECT {projection} FROM {table}"))
                .await
                .map(|description| {
                    description
                        .columns
                        .iter()
                        .map(|column| column.type_oid)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let mut header = BytesMut::new();
            header.extend_from_slice(COPY_BINARY_SIGNATURE);
            header.put_i32(0);
            header.put_i32(0);
            BackendMessage::CopyData {
                data: header.freeze(),
            }
            .encode(buf);

            for row in &rows {
                let mut tuple = BytesMut::new();
                tuple.put_i16(row.len() as i16);
                for (index, value) in row.iter().enumerate() {
                    match value {
                        None => tuple.put_i32(-1),
                        Some(text) => {
                            let oid = types.get(index).copied().unwrap_or(type_oids::TEXT);
                            let encoded = Self::encode_binary_value(text, oid);
                            tuple.put_i32(encoded.len() as i32);
                            tuple.extend_from_slice(&encoded);
                        }
                    }
                }
                BackendMessage::CopyData {
                    data: tuple.freeze(),
                }
                .encode(buf);
            }

            let mut trailer = BytesMut::new();
            trailer.put_i16(-1);
            BackendMessage::CopyData {
                data: trailer.freeze(),
            }
            .encode(buf);
            BackendMessage::CopyDone.encode(buf);
            return;
        }

        for row in &rows {
            let line = if csv {
                row.iter()
                    .map(|value| match value {
                        // CSV spells NULL as an empty field, not `\N`.
                        None => String::new(),
                        Some(text) => Self::csv_field(text),
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            } else {
                row.iter()
                    .map(|value| match value {
                        // `\N` is how the text format spells NULL, and is why
                        // a literal backslash has to be escaped.
                        None => "\\N".to_string(),
                        Some(text) => text
                            .replace('\\', "\\\\")
                            .replace('\t', "\\t")
                            .replace('\n', "\\n")
                            .replace('\r', "\\r"),
                    })
                    .collect::<Vec<_>>()
                    .join("\t")
            };
            BackendMessage::CopyData {
                data: bytes::Bytes::from(format!("{line}\n")),
            }
            .encode(buf);
        }

        BackendMessage::CopyDone.encode(buf);
        BackendMessage::CommandComplete {
            tag: format!("COPY {}", rows.len()),
        }
        .encode(buf);
    }

    /// Put the session into copy-in mode and invite the client to stream.
    async fn begin_copy_from_stdin(
        &mut self,
        table: &str,
        columns: Vec<String>,
        binary: bool,
        csv: bool,
        buf: &mut BytesMut,
    ) {
        // Column order has to be known before the first row arrives; when the
        // statement did not name any, the table's own order is used.
        let schema = match self.query_engine.table_schema(table).await {
            Ok(schema) => schema,
            Err(e) => {
                self.send_error_for(buf, &e);
                return;
            }
        };

        let columns = if columns.is_empty() {
            match &schema {
                Some(schema) => schema.columns.iter().map(|c| c.name.clone()).collect(),
                None => {
                    self.send_error(buf, &format!("Table '{table}' does not exist"));
                    return;
                }
            }
        } else {
            columns
        };

        use crate::protocols::postgres_wire::persistent_storage::ColumnType;
        let numeric_columns: Vec<bool> = columns
            .iter()
            .map(|name| {
                schema.as_ref().is_some_and(|schema| {
                    schema
                        .columns
                        .iter()
                        .find(|c| c.name.eq_ignore_ascii_case(name))
                        .is_some_and(|c| {
                            matches!(
                                c.data_type,
                                ColumnType::Serial
                                    | ColumnType::Integer
                                    | ColumnType::BigInt
                                    | ColumnType::Double
                                    | ColumnType::Boolean
                            )
                        })
                })
            })
            .collect();

        // Type OIDs are needed per column to decode a binary stream.
        let column_type_oids: Vec<i32> = columns
            .iter()
            .map(|name| {
                schema
                    .as_ref()
                    .and_then(|schema| {
                        schema
                            .columns
                            .iter()
                            .find(|c| c.name.eq_ignore_ascii_case(name))
                    })
                    .map_or(type_oids::TEXT, |column| {
                        super::query_engine::column_type_oid(&column.data_type)
                    })
            })
            .collect();

        BackendMessage::CopyInResponse {
            format: i8::from(binary),
            column_formats: vec![i16::from(binary); columns.len()],
        }
        .encode(buf);

        self.copy_in = Some(CopyInState {
            binary,
            csv,
            pending: BytesMut::new(),
            header_seen: false,
            simple_protocol: self.copy_in_is_simple,
            table: table.to_string(),
            column_type_oids,
            numeric_columns,
            columns,
            partial: Vec::new(),
            rows: 0,
            failure: None,
        });
    }

    /// Consume one chunk of a binary copy-in stream.
    ///
    /// The stream is a fixed header followed by length-prefixed tuples, so a
    /// chunk may end anywhere; whatever does not form a whole tuple is kept
    /// for the next message.
    async fn handle_binary_copy_data(
        &mut self,
        data: &[u8],
        _buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        {
            let Some(state) = self.copy_in.as_mut() else {
                return Ok(());
            };
            state.pending.extend_from_slice(data);
        }

        loop {
            let Some(state) = self.copy_in.as_mut() else {
                return Ok(());
            };

            if !state.header_seen {
                // Signature, flags and the header extension's length.
                const HEADER: usize = 11 + 4 + 4;
                if state.pending.len() < HEADER {
                    return Ok(());
                }
                if &state.pending[..11] != COPY_BINARY_SIGNATURE {
                    state.failure = Some("COPY binary stream has a bad signature".to_string());
                    state.pending.clear();
                    return Ok(());
                }
                let extension = i32::from_be_bytes([
                    state.pending[15],
                    state.pending[16],
                    state.pending[17],
                    state.pending[18],
                ]) as usize;
                if state.pending.len() < HEADER + extension {
                    return Ok(());
                }
                let _ = state.pending.split_to(HEADER + extension);
                state.header_seen = true;
                continue;
            }

            if state.pending.len() < 2 {
                return Ok(());
            }
            let fields = i16::from_be_bytes([state.pending[0], state.pending[1]]);
            if fields < 0 {
                // The end-of-data trailer.
                let _ = state.pending.split_to(2);
                return Ok(());
            }

            // Measure the whole tuple before consuming any of it, so a chunk
            // that stops mid-value is simply waited on.
            let mut offset = 2usize;
            let mut lengths = Vec::with_capacity(fields as usize);
            for _ in 0..fields {
                if state.pending.len() < offset + 4 {
                    return Ok(());
                }
                let length = i32::from_be_bytes([
                    state.pending[offset],
                    state.pending[offset + 1],
                    state.pending[offset + 2],
                    state.pending[offset + 3],
                ]);
                offset += 4;
                if length >= 0 {
                    if state.pending.len() < offset + length as usize {
                        return Ok(());
                    }
                    offset += length as usize;
                }
                lengths.push(length);
            }

            let tuple = state.pending.split_to(offset);
            let oids = state.column_type_oids.clone();
            let mut values = Vec::with_capacity(lengths.len());
            let mut cursor = 2usize;
            for (index, length) in lengths.into_iter().enumerate() {
                cursor += 4;
                if length < 0 {
                    values.push(None);
                    continue;
                }
                let raw = &tuple[cursor..cursor + length as usize];
                cursor += length as usize;
                let oid = oids.get(index).copied().unwrap_or(type_oids::TEXT);
                match Self::decode_binary_parameter(raw, oid) {
                    Ok(text) => values.push(Some(text)),
                    Err(e) => {
                        if let Some(state) = self.copy_in.as_mut() {
                            if state.failure.is_none() {
                                state.failure = Some(e);
                            }
                        }
                        values.push(None);
                    }
                }
            }

            // The decoded values are written through the same path a text row
            // takes, so quoting and constraint checks behave identically.
            let line = values
                .into_iter()
                .map(|value| match value {
                    None => "\\N".to_string(),
                    Some(text) => text
                        .replace('\\', "\\\\")
                        .replace('\t', "\\t")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r"),
                })
                .collect::<Vec<_>>()
                .join("\t");
            self.insert_copy_line(&line).await?;
        }
    }

    /// Consume one chunk of copy-in data.
    async fn handle_copy_data(&mut self, data: &[u8], buf: &mut BytesMut) -> ProtocolResult<()> {
        if self.copy_in.is_none() {
            self.send_error(buf, "CopyData received while not in copy-in mode");
            return Ok(());
        }

        if self.copy_in.as_ref().is_some_and(|state| state.binary) {
            return self.handle_binary_copy_data(data, buf).await;
        }

        // Chunks split rows arbitrarily, so only whole lines are consumed and
        // the remainder is carried to the next message.
        let mut pending = {
            let state = self.copy_in.as_mut().expect("checked above");
            state.partial.extend_from_slice(data);
            std::mem::take(&mut state.partial)
        };

        let mut consumed = 0usize;
        while let Some(newline) = pending[consumed..].iter().position(|b| *b == b'\n') {
            let line_end = consumed + newline;
            let line = String::from_utf8_lossy(&pending[consumed..line_end]).into_owned();
            consumed = line_end + 1;
            self.insert_copy_line(line.trim_end_matches('\r')).await?;
        }

        pending.drain(..consumed);
        if let Some(state) = self.copy_in.as_mut() {
            state.partial = pending;
        }
        Ok(())
    }

    /// Insert one text-format COPY line.
    async fn insert_copy_line(&mut self, line: &str) -> ProtocolResult<()> {
        // The end-of-data marker is a line containing only `\.`.
        if line.is_empty() || line == "\\." {
            return Ok(());
        }

        let Some(state) = self.copy_in.as_ref() else {
            return Ok(());
        };

        // CSV separates on commas and honours quotes; the text format
        // separates on tabs and uses backslash escapes. Reading a CSV line the
        // second way gave one field and a column-count mismatch.
        let fields: Vec<String> = if state.csv {
            Self::split_csv_line(line)
        } else {
            line.split('\t').map(str::to_string).collect()
        };
        let csv = state.csv;

        let values: Vec<String> = fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let field = field.as_str();
                // CSV spells NULL as an empty unquoted field.
                if (csv && field.is_empty()) || (!csv && field == "\\N") {
                    return "NULL".to_string();
                }
                let unescaped = if csv {
                    field.to_string()
                } else {
                    field
                        .replace("\\t", "\t")
                        .replace("\\n", "\n")
                        .replace("\\r", "\r")
                        .replace("\\\\", "\\")
                };

                // Bare only where the column takes a bare literal *and* the
                // value really is one; anything else is quoted so a stray field
                // cannot become syntax.
                let numeric = state.numeric_columns.get(index).copied().unwrap_or(false);
                if numeric && Self::is_safe_bare_literal(&unescaped) {
                    unescaped
                } else {
                    format!("'{}'", unescaped.replace('\'', "''"))
                }
            })
            .collect();

        let statement = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            state.table,
            state.columns.join(", "),
            values.join(", ")
        );

        // Each copied line is an ordinary INSERT, so it records itself in the
        // undo log the same way. Without this a `COPY` inside a transaction
        // block fell back to a whole-table copy, whose rollback reverted a
        // concurrent session's writes to that table.
        self.snapshot_before_write(&statement).await;

        match self.query_engine.execute_query(&statement).await {
            Ok(_) => {
                if let Some(state) = self.copy_in.as_mut() {
                    state.rows += 1;
                }
                Ok(())
            }
            // Recorded rather than returned: an error raised here would be
            // written to a client that is streaming data and expecting no
            // messages at all, which desynchronises the connection. It is
            // reported when the stream ends.
            Err(e) => {
                if let Some(state) = self.copy_in.as_mut() {
                    if state.failure.is_none() {
                        state.failure = Some(e.to_string());
                    }
                }
                Ok(())
            }
        }
    }

    /// Finish a copy-in stream.
    async fn handle_copy_done(&mut self, buf: &mut BytesMut) -> ProtocolResult<()> {
        let Some(state) = self.copy_in.take() else {
            self.send_error(buf, "CopyDone received while not in copy-in mode");
            return Ok(());
        };

        match &state.failure {
            Some(failure) => self.send_error(buf, failure),
            None => BackendMessage::CommandComplete {
                tag: format!("COPY {}", state.rows),
            }
            .encode(buf),
        }
        self.finish_copy_statement(state.simple_protocol, buf);
        Ok(())
    }

    /// Send at most `max_rows` rows of a portal, starting at `already_sent`.
    ///
    /// `max_rows == 0` means "no limit", per the protocol. When rows remain
    /// after the limit is reached the reply is `PortalSuspended` rather than
    /// `CommandComplete`: that is what tells the client to ask for the next
    /// page instead of concluding the result set ended.
    async fn send_portal_page(
        &mut self,
        portal: &str,
        columns: &[String],
        rows: &[Vec<Option<String>>],
        already_sent: usize,
        max_rows: i32,
        buf: &mut BytesMut,
    ) {
        let _ = columns;
        let limit = if max_rows <= 0 {
            rows.len().saturating_sub(already_sent)
        } else {
            (max_rows as usize).min(rows.len().saturating_sub(already_sent))
        };

        let formats = self
            .portal_result_formats
            .get(portal)
            .cloned()
            .unwrap_or_default();
        let column_types = self
            .portals
            .get(portal)
            .and_then(|(statement, _)| self.statement_columns.get(statement))
            .cloned()
            .unwrap_or_default();

        // Binary output is asked for in `Bind`, which a client may send
        // without ever issuing `Describe` — and `Describe` was the only thing
        // that recorded a column's type. Without it every value fell back to
        // text, so a client that asked for binary silently got characters.
        let wants_binary = formats.contains(&1);
        let column_types = if wants_binary && column_types.is_empty() {
            self.column_types_for_portal(portal).await
        } else {
            column_types
        };

        for row in rows.iter().skip(already_sent).take(limit) {
            let values: Vec<Option<bytes::Bytes>> = row
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let text = value.as_ref()?;
                    let binary = match formats.len() {
                        0 => false,
                        1 => formats[0] == 1,
                        _ => formats.get(index).is_some_and(|f| *f == 1),
                    };
                    if !binary {
                        return Some(bytes::Bytes::from(text.clone()));
                    }
                    let type_oid = column_types.get(index).copied().unwrap_or(type_oids::TEXT);
                    Some(Self::encode_binary_value(text, type_oid))
                })
                .collect();
            BackendMessage::DataRow { values }.encode(buf);
        }

        let sent = already_sent + limit;
        if sent < rows.len() {
            if let Some(state) = self.portal_rows.get_mut(portal) {
                state.sent = sent;
            }
            BackendMessage::PortalSuspended.encode(buf);
        } else {
            self.portal_rows.remove(portal);
            BackendMessage::CommandComplete {
                tag: format!("SELECT {sent}"),
            }
            .encode(buf);
        }
    }

    /// The column types of a portal's statement, described on demand.
    ///
    /// Only consulted when binary output was asked for and nothing has
    /// described the statement yet, so an ordinary text query pays nothing.
    async fn column_types_for_portal(&mut self, portal: &str) -> Vec<i32> {
        let Some(statement) = self
            .portals
            .get(portal)
            .map(|(statement, _)| statement.clone())
        else {
            return Vec::new();
        };
        let Some(sql) = self.prepared_statements.get(&statement).cloned() else {
            return Vec::new();
        };
        let Ok(description) = self.query_engine.describe_statement(&sql).await else {
            return Vec::new();
        };
        let types: Vec<i32> = description.columns.iter().map(|c| c.type_oid).collect();
        self.statement_columns.insert(statement, types.clone());
        types
    }

    /// Encode one value in PostgreSQL's binary format for `type_oid`.
    ///
    /// The engine holds every value as text, so this parses and re-encodes.
    /// A value that will not parse as its declared type is sent as its text
    /// bytes: that is what the value actually is, and it keeps a type
    /// mismatch in the catalogue from corrupting unrelated columns in the row.
    fn encode_binary_value(text: &str, type_oid: i32) -> bytes::Bytes {
        fn bytes_of(vector: Vec<u8>) -> bytes::Bytes {
            bytes::Bytes::from(vector)
        }

        match type_oid {
            type_oids::BOOL => {
                let value = matches!(
                    text.to_ascii_lowercase().as_str(),
                    "t" | "true" | "1" | "yes" | "on"
                );
                bytes_of(vec![u8::from(value)])
            }
            type_oids::INT2 => text
                .parse::<i16>()
                .map(|n| bytes_of(n.to_be_bytes().to_vec()))
                .unwrap_or_else(|_| bytes::Bytes::from(text.to_string())),
            type_oids::INT4 => text
                .parse::<i32>()
                .map(|n| bytes_of(n.to_be_bytes().to_vec()))
                .unwrap_or_else(|_| bytes::Bytes::from(text.to_string())),
            type_oids::INT8 => text
                .parse::<i64>()
                .map(|n| bytes_of(n.to_be_bytes().to_vec()))
                .unwrap_or_else(|_| bytes::Bytes::from(text.to_string())),
            type_oids::FLOAT4 => text
                .parse::<f32>()
                .map(|n| bytes_of(n.to_be_bytes().to_vec()))
                .unwrap_or_else(|_| bytes::Bytes::from(text.to_string())),
            type_oids::FLOAT8 => text
                .parse::<f64>()
                .map(|n| bytes_of(n.to_be_bytes().to_vec()))
                .unwrap_or_else(|_| bytes::Bytes::from(text.to_string())),
            // text, json, uuid and anything unrecognised are the same bytes in
            // both formats.
            _ => bytes::Bytes::from(text.to_string()),
        }
    }

    /// Send rows and the command tag, without a row description.
    ///
    /// This is what `Execute` must send: in the extended query protocol the row
    /// description belongs to `Describe`, and repeating it here makes
    /// conforming clients fail.
    fn send_query_result_without_description(&self, result: &QueryResult, buf: &mut BytesMut) {
        match result {
            QueryResult::Select { columns: _, rows } => {
                for row in rows {
                    let values: Vec<Option<bytes::Bytes>> = row
                        .iter()
                        .map(|v| v.as_ref().map(|s| bytes::Bytes::from(s.clone())))
                        .collect();
                    BackendMessage::DataRow { values }.encode(buf);
                }

                BackendMessage::CommandComplete {
                    tag: format!("SELECT {}", rows.len()),
                }
                .encode(buf);
            }
            QueryResult::Insert { count } => {
                BackendMessage::CommandComplete {
                    tag: format!("INSERT 0 {count}"),
                }
                .encode(buf);
            }
            QueryResult::Update { count } => {
                BackendMessage::CommandComplete {
                    tag: format!("UPDATE {count}"),
                }
                .encode(buf);
            }
            QueryResult::Delete { count } => {
                BackendMessage::CommandComplete {
                    tag: format!("DELETE {count}"),
                }
                .encode(buf);
            }
            QueryResult::Merge {
                count,
                rows,
                columns,
            } => {
                // If rows are present (RETURNING clause), we need to send RowDescription and DataRow
                if !rows.is_empty() {
                    let fields: Vec<FieldDescription> = columns
                        .iter()
                        .map(|col| FieldDescription {
                            name: col.clone(),
                            table_oid: 0,
                            column_id: 0,
                            type_oid: type_oids::TEXT, // Default to TEXT
                            type_size: -1,
                            type_modifier: -1,
                            format: 0,
                        })
                        .collect();

                    BackendMessage::RowDescription { fields }.encode(buf);

                    for row in rows {
                        let values: Vec<Option<bytes::Bytes>> = row
                            .iter()
                            .map(|v| v.as_ref().map(|s| bytes::Bytes::from(s.clone())))
                            .collect();
                        BackendMessage::DataRow { values }.encode(buf);
                    }
                }

                BackendMessage::CommandComplete {
                    tag: format!("MERGE {count}"),
                }
                .encode(buf);
            }
            QueryResult::Set { .. } => {
                BackendMessage::CommandComplete {
                    tag: "SET".to_string(),
                }
                .encode(buf);
            }
        }
    }

    /// Run a legacy fast-path function call.
    ///
    /// The OID names a function in `pg_proc`; this server publishes its own
    /// there with OIDs in PostgreSQL's user range, so a client that looks one
    /// up can call it this way. An OID it did not publish is refused by
    /// number, because guessing which built-in a number meant would have the
    /// client silently calling something else.
    async fn handle_function_call(
        &mut self,
        oid: i32,
        args: &[Option<bytes::Bytes>],
        arg_formats: &[i16],
        result_format: i16,
        buf: &mut BytesMut,
    ) -> ProtocolResult<()> {
        let found = self.query_engine.function_for_oid(i64::from(oid)).await?;

        let Some((name, parameters, return_type)) = found else {
            self.send_error(
                buf,
                &format!(
                    "function with OID {oid} does not exist; \
                     look it up in pg_proc or call it from a query"
                ),
            );
            BackendMessage::ReadyForQuery {
                status: self.transaction_status(),
            }
            .encode(buf);
            return Ok(());
        };

        // Arguments arrive as bytes in whichever format the client chose.
        // Their declared types come from the same catalogue entry the client
        // read the OID from, which is what makes decoding a binary one
        // possible rather than a guess.
        let inputs = super::plpgsql_function::inputs(&parameters);
        let mut rendered = Vec::with_capacity(args.len());
        for (index, arg) in args.iter().enumerate() {
            let declared = inputs
                .get(index)
                .map_or("", |parameter| parameter.sql_type.as_str());
            match super::fastpath::decode_argument(
                arg.as_deref(),
                Self::format_at(arg_formats, index),
                declared,
            ) {
                Ok(value) => rendered.push(value),
                Err(e) => {
                    self.send_error_for(buf, &e);
                    BackendMessage::ReadyForQuery {
                        status: self.transaction_status(),
                    }
                    .encode(buf);
                    return Ok(());
                }
            }
        }

        let call = format!("SELECT {name}({})", rendered.join(", "));
        match self.query_engine.execute_query(&call).await {
            Ok(super::query_engine::QueryResult::Select { rows, .. }) => {
                let value = rows
                    .into_iter()
                    .next()
                    .and_then(|row| row.into_iter().next())
                    .flatten();
                match super::fastpath::encode_result(value, result_format, &return_type) {
                    Ok(val) => BackendMessage::FunctionCallResponse { val }.encode(buf),
                    Err(e) => self.send_error_for(buf, &e),
                }
            }
            Ok(_) => {
                BackendMessage::FunctionCallResponse { val: None }.encode(buf);
            }
            Err(e) => {
                self.send_error_for(buf, &e);
            }
        }

        BackendMessage::ReadyForQuery {
            status: self.transaction_status(),
        }
        .encode(buf);
        Ok(())
    }

    /// The format code that applies to argument `index`.
    ///
    /// None means every argument is text; one means it applies to all of them;
    /// otherwise there is one per argument. Reading the array as one-per-
    /// argument regardless would misread the common single-code case.
    fn format_at(formats: &[i16], index: usize) -> i16 {
        match formats {
            [] => 0,
            [only] => *only,
            many => many.get(index).copied().unwrap_or(0),
        }
    }

    /// Handle SSL request
    /// Answer an `SSLRequest` that reached the message loop.
    ///
    /// TLS is negotiated by the listener before this handler ever runs, so an
    /// `SSLRequest` arriving here is a second one on an already-established
    /// session. `N` is the correct answer: whatever transport the connection
    /// has is already fixed.
    async fn handle_ssl_request(&mut self, buf: &mut BytesMut) -> ProtocolResult<()> {
        buf.put_u8(b'N');
        Ok(())
    }

    /// Report an error under the SQLSTATE it carries.
    ///
    /// An error that knows its own code keeps it — a `RAISE EXCEPTION` is
    /// `P0001` whatever its text says, and no reading of that text would
    /// reveal it.
    fn send_error_for(&mut self, buf: &mut BytesMut, error: &ProtocolError) {
        // Anything the client already pipelined behind this is discarded until
        // it synchronises — in the extended protocol only, where `Sync` is the
        // synchronisation point.
        self.skip_until_sync |= self.handling_extended;
        let code = super::sqlstate::of(error);
        let reported = error.to_string();
        let reported = reported
            .split_once("PostgreSQL protocol error: ")
            .map_or(reported.as_str(), |(_, rest)| rest);

        let mut fields = HashMap::new();
        fields.insert(b'S', "ERROR".to_string());
        fields.insert(b'C', code.to_string());
        fields.insert(b'M', reported.to_string());
        BackendMessage::ErrorResponse { fields }.encode(buf);
    }

    /// Send error response
    ///
    /// The SQLSTATE is classified rather than always `XX000`: a driver reading
    /// `internal_error` for a duplicate key cannot tell a constraint it should
    /// handle from a backend that fell over.
    fn send_error(&mut self, buf: &mut BytesMut, message: &str) {
        self.skip_until_sync |= self.handling_extended;
        // The transport's name is not part of the error. `PostgreSQL protocol
        // error: relation does not exist` is our plumbing showing through.
        let reported = message
            .split_once("PostgreSQL protocol error: ")
            .map_or(message, |(_, rest)| rest);

        let mut fields = HashMap::new();
        fields.insert(b'S', "ERROR".to_string());
        fields.insert(b'C', super::sqlstate::classify(reported).to_string());
        fields.insert(b'M', reported.to_string());

        BackendMessage::ErrorResponse { fields }.encode(buf);
    }
}

impl Default for PostgresWireProtocol {
    fn default() -> Self {
        Self::new()
    }
}

// Add rand dependency for secret_key generation
use rand::RngExt;
impl PostgresWireProtocol {
    /// Generate a random cancel key
    /// PostgreSQL 18 (protocol 3.2): Supports 4-256 bytes
    /// Default: 4 bytes for backward compatibility with protocol 3.0 clients
    fn random_secret_key() -> Vec<u8> {
        let mut key = vec![0u8; 4]; // 4 bytes for compatibility
        rand::rng().fill(&mut key[..]);
        key
    }
}

#[cfg(test)]
mod extended_protocol_tests {
    use super::*;

    fn param(text: &str) -> Option<bytes::Bytes> {
        Some(bytes::Bytes::from(text.to_string()))
    }

    #[test]
    fn placeholders_are_counted_by_highest_position_not_occurrences() {
        assert_eq!(PostgresWireProtocol::count_placeholders("SELECT 1"), 0);
        assert_eq!(
            PostgresWireProtocol::count_placeholders("SELECT * FROM t WHERE a = $1 AND b = $1"),
            1
        );
        assert_eq!(
            PostgresWireProtocol::count_placeholders("SELECT * FROM t WHERE a = $2 AND b = $1"),
            2
        );
    }

    #[test]
    fn a_dollar_inside_a_string_literal_is_not_a_placeholder() {
        assert_eq!(
            PostgresWireProtocol::count_placeholders("SELECT '$1' FROM t"),
            0
        );
        assert_eq!(
            PostgresWireProtocol::count_placeholders("SELECT '$5' FROM t WHERE a = $1"),
            1
        );
    }

    #[test]
    fn parameters_are_substituted_in_position_order() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT * FROM t WHERE a = $1 AND b = $2",
            &[param("one"), param("two")],
            &[],
        )
        .expect("both parameters are bound");
        assert_eq!(bound, "SELECT * FROM t WHERE a = 'one' AND b = 'two'");
    }

    #[test]
    fn a_repeated_placeholder_uses_the_same_value_each_time() {
        let bound = PostgresWireProtocol::bind_parameters("SELECT $1, $1", &[param("x")], &[])
            .expect("bound");
        assert_eq!(bound, "SELECT 'x', 'x'");
    }

    #[test]
    fn a_null_parameter_becomes_sql_null_not_an_empty_string() {
        let bound =
            PostgresWireProtocol::bind_parameters("SELECT $1", &[None], &[]).expect("bound");
        assert_eq!(bound, "SELECT NULL");
    }

    /// A parameter is data. Quotes and statement terminators inside one must not
    /// become syntax.
    #[test]
    fn quotes_in_a_parameter_are_escaped_rather_than_ending_the_literal() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT * FROM t WHERE name = $1",
            &[param("O'Brien")],
            &[],
        )
        .expect("bound");
        assert_eq!(bound, "SELECT * FROM t WHERE name = 'O''Brien'");
    }

    #[test]
    fn an_injection_attempt_in_a_parameter_stays_inside_the_literal() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT * FROM t WHERE name = $1",
            &[param("x'; DROP TABLE users; --")],
            &[],
        )
        .expect("bound");
        assert_eq!(
            bound,
            "SELECT * FROM t WHERE name = 'x''; DROP TABLE users; --'"
        );
        // Exactly one literal: an odd number of quotes would mean the value
        // escaped into statement syntax.
        assert_eq!(bound.matches('\'').count() % 2, 0);
    }

    #[test]
    fn a_placeholder_inside_a_literal_is_left_untouched() {
        let bound = PostgresWireProtocol::bind_parameters("SELECT '$1', $1", &[param("v")], &[])
            .expect("bound");
        assert_eq!(bound, "SELECT '$1', 'v'");
    }

    /// Running with a literal `$1` still in the statement would quietly return
    /// the wrong rows, so an unbound reference must fail loudly.
    #[test]
    fn referencing_an_unbound_parameter_is_an_error() {
        let error = PostgresWireProtocol::bind_parameters("SELECT $2", &[param("only-one")], &[])
            .expect_err("only one parameter was bound");
        assert!(
            error.to_string().contains("$2"),
            "the message should name the missing parameter: {error}"
        );
    }

    #[test]
    fn a_statement_without_placeholders_is_unchanged() {
        let bound =
            PostgresWireProtocol::bind_parameters("SELECT 1 FROM t", &[], &[]).expect("bound");
        assert_eq!(bound, "SELECT 1 FROM t");
    }

    #[test]
    fn binary_scalars_decode_to_their_text_form() {
        use super::super::messages::type_oids;

        let decode = PostgresWireProtocol::decode_binary_parameter;
        assert_eq!(decode(&2i32.to_be_bytes(), type_oids::INT4).unwrap(), "2");
        assert_eq!(
            decode(&(-7i64).to_be_bytes(), type_oids::INT8).unwrap(),
            "-7"
        );
        assert_eq!(
            decode(&300i16.to_be_bytes(), type_oids::INT2).unwrap(),
            "300"
        );
        assert_eq!(decode(&[1], type_oids::BOOL).unwrap(), "true");
        assert_eq!(decode(&[0], type_oids::BOOL).unwrap(), "false");
        assert_eq!(decode(b"hello", type_oids::TEXT).unwrap(), "hello");
        assert_eq!(
            decode(&1.5f64.to_be_bytes(), type_oids::FLOAT8).unwrap(),
            "1.5"
        );
    }

    /// Misreading the bytes would substitute a wrong value with no error, so a
    /// wrong-width payload and an unknown type must both be refused.
    #[test]
    fn malformed_or_unknown_binary_parameters_are_refused() {
        use super::super::messages::type_oids;

        let decode = PostgresWireProtocol::decode_binary_parameter;
        assert!(decode(&[1, 2], type_oids::INT4).is_err(), "wrong width");
        assert!(decode(&[2], type_oids::BOOL).is_err(), "not 0 or 1");
        assert!(
            decode(&[0; 8], type_oids::BYTEA).is_err(),
            "unsupported type"
        );
    }

    #[test]
    fn describing_a_parameterised_statement_neutralises_placeholders() {
        use crate::protocols::postgres_wire::query_engine::QueryEngine;

        assert_eq!(
            QueryEngine::placeholders_as_null_for_test("SELECT a FROM t WHERE b = $1 AND c = $22"),
            "SELECT a FROM t WHERE b = NULL AND c = NULL"
        );
        assert_eq!(
            QueryEngine::placeholders_as_null_for_test("SELECT '$1' FROM t"),
            "SELECT '$1' FROM t"
        );
    }

    #[test]
    fn comparisons_pair_a_placeholder_with_the_column_beside_it() {
        use crate::protocols::postgres_wire::query_engine::QueryEngine;

        let pairs = QueryEngine::placeholder_comparisons_for_test(
            "SELECT * FROM t WHERE id = $1 AND score >= $2 AND name LIKE $3",
        );
        assert_eq!(
            pairs,
            vec![
                (1, "id".to_string()),
                (2, "score".to_string()),
                (3, "name".to_string())
            ]
        );
    }

    #[test]
    fn a_placeholder_that_is_not_compared_to_a_column_is_left_alone() {
        use crate::protocols::postgres_wire::query_engine::QueryEngine;

        assert!(QueryEngine::placeholder_comparisons_for_test("SELECT $1").is_empty());
    }

    #[test]
    fn numeric_parameters_are_spliced_without_quotes() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT * FROM t WHERE id = $1",
            &[param("42")],
            &[type_oids::INT4],
        )
        .expect("bound");
        // Quoting this would compare an integer column against a string and
        // match nothing.
        assert_eq!(bound, "SELECT * FROM t WHERE id = 42");
    }

    #[test]
    fn boolean_parameters_are_spliced_without_quotes() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT $1",
            &[param("true")],
            &[type_oids::BOOL],
        )
        .expect("bound");
        assert_eq!(bound, "SELECT true");
    }

    #[test]
    fn text_parameters_stay_quoted_even_when_they_look_numeric() {
        let bound =
            PostgresWireProtocol::bind_parameters("SELECT $1", &[param("42")], &[type_oids::TEXT])
                .expect("bound");
        assert_eq!(bound, "SELECT '42'");
    }

    /// A value claiming to be numeric but carrying SQL must never be spliced
    /// bare, whatever the declared type says.
    #[test]
    fn a_non_numeric_value_declared_numeric_is_still_quoted() {
        let bound = PostgresWireProtocol::bind_parameters(
            "SELECT * FROM t WHERE id = $1",
            &[param("1; DROP TABLE users")],
            &[type_oids::INT4],
        )
        .expect("bound");
        assert_eq!(bound, "SELECT * FROM t WHERE id = '1; DROP TABLE users'");
    }

    #[test]
    fn binary_encoding_matches_postgres_wire_widths() {
        use super::super::messages::type_oids;

        let encode = PostgresWireProtocol::encode_binary_value;
        assert_eq!(encode("5", type_oids::INT4).as_ref(), &5i32.to_be_bytes());
        assert_eq!(
            encode("-7", type_oids::INT8).as_ref(),
            &(-7i64).to_be_bytes()
        );
        assert_eq!(
            encode("300", type_oids::INT2).as_ref(),
            &300i16.to_be_bytes()
        );
        assert_eq!(
            encode("1.5", type_oids::FLOAT8).as_ref(),
            &1.5f64.to_be_bytes()
        );
        assert_eq!(encode("true", type_oids::BOOL).as_ref(), &[1u8]);
        assert_eq!(encode("f", type_oids::BOOL).as_ref(), &[0u8]);
    }

    /// Text is identical in both formats, so it must not be transformed.
    #[test]
    fn text_is_unchanged_by_binary_encoding() {
        use super::super::messages::type_oids;

        assert_eq!(
            PostgresWireProtocol::encode_binary_value("hello", type_oids::TEXT).as_ref(),
            b"hello"
        );
    }

    /// A value that does not parse as its declared type falls back to its own
    /// bytes rather than emitting a wrong-width field that would desynchronise
    /// the client's read of the rest of the row.
    #[test]
    fn an_unparseable_value_falls_back_to_its_text_bytes() {
        use super::super::messages::type_oids;

        assert_eq!(
            PostgresWireProtocol::encode_binary_value("not-a-number", type_oids::INT4).as_ref(),
            b"not-a-number"
        );
    }
}
