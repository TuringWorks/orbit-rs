//! MongoDB protocol server implementation
//!
//! Provides a MongoDB-compatible wire protocol server with full CRUD support.

use super::protocol::{MongoCodec, MongoHeader, MongoMessage, MsgSection, OP_MSG, OP_REPLY};
use super::storage::DocumentStore;
use bson::{doc, Bson, Document};
use futures::{SinkExt, StreamExt};
use orbit_shared::OrbitResult;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;
use tracing::{debug, error, info, warn};

pub struct MongoDbServer {
    address: String,
    store: Arc<DocumentStore>,
}

impl MongoDbServer {
    pub fn new(address: String) -> Self {
        Self {
            address,
            store: Arc::new(DocumentStore::new()),
        }
    }

    pub fn with_store(address: String, store: Arc<DocumentStore>) -> Self {
        Self { address, store }
    }

    pub async fn run(&self) -> OrbitResult<()> {
        let listener = TcpListener::bind(&self.address).await?;
        info!("MongoDB server listening on {}", self.address);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    debug!("New MongoDB connection from {}", addr);
                    let store = self.store.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(socket, store).await {
                            error!("MongoDB connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept MongoDB connection: {}", e);
                }
            }
        }
    }
}

async fn handle_connection(socket: TcpStream, store: Arc<DocumentStore>) -> OrbitResult<()> {
    info!("MongoDB: Starting connection handler");

    let mut framed = Framed::new(socket, MongoCodec::new());

    while let Some(result) = framed.next().await {
        match result {
            Ok(message) => {
                let response = handle_message(message, &store).await;
                if let Some(reply) = response {
                    framed.send(reply).await?;
                }
            }
            Err(e) => {
                error!("Protocol error: {}", e);
                return Err(e);
            }
        }
    }

    Ok(())
}

async fn handle_message(message: MongoMessage, store: &DocumentStore) -> Option<MongoMessage> {
    match message {
        MongoMessage::Query {
            header,
            full_collection_name,
            query,
            number_to_return,
            number_to_skip,
            ..
        } => {
            debug!("Received OP_QUERY: {} {:?}", full_collection_name, query);

            // Handle handshake (isMaster / hello / ismaster)
            if full_collection_name.ends_with(".$cmd") {
                if query.contains_key("isMaster")
                    || query.contains_key("ismaster")
                    || query.contains_key("hello")
                {
                    return Some(make_reply(header.request_id, vec![handshake_response()]));
                } else {
                    return Some(make_reply(header.request_id, vec![doc! { "ok": 1.0 }]));
                }
            }

            // Parse database and collection from full_collection_name
            let parts: Vec<&str> = full_collection_name.splitn(2, '.').collect();
            if parts.len() == 2 {
                let db = parts[0];
                let collection = parts[1];
                let limit = if number_to_return > 0 {
                    Some(number_to_return as i64)
                } else {
                    None
                };
                let skip = if number_to_skip > 0 {
                    Some(number_to_skip as i64)
                } else {
                    None
                };

                let (_, docs) = store.find(db, collection, &query, limit, skip, None).await;
                return Some(make_reply(header.request_id, docs));
            }

            Some(make_reply(header.request_id, vec![]))
        }

        MongoMessage::Msg {
            header, sections, ..
        } => {
            debug!("Received OP_MSG");

            // Find the body section
            let mut command_doc = doc! {};
            let mut doc_sequence: Vec<Document> = Vec::new();

            for section in &sections {
                match section {
                    MsgSection::Body(doc) => {
                        command_doc = doc.clone();
                    }
                    MsgSection::DocumentSequence { documents, .. } => {
                        doc_sequence = documents.clone();
                    }
                }
            }

            debug!("Command: {:?}", command_doc);

            let db = command_doc
                .get_str("$db")
                .unwrap_or("test")
                .to_string();

            let response_doc = handle_command(&db, command_doc, doc_sequence, store).await;

            Some(MongoMessage::Msg {
                header: MongoHeader {
                    message_length: 0,
                    request_id: header.request_id + 1,
                    response_to: header.request_id,
                    op_code: OP_MSG,
                },
                flag_bits: 0,
                sections: vec![MsgSection::Body(response_doc)],
                checksum: None,
            })
        }

        _ => {
            debug!("Received unknown or unsupported message");
            None
        }
    }
}

async fn handle_command(
    db: &str,
    command: Document,
    doc_sequence: Vec<Document>,
    store: &DocumentStore,
) -> Document {
    // Handshake commands
    if command.contains_key("isMaster")
        || command.contains_key("ismaster")
        || command.contains_key("hello")
    {
        return handshake_response();
    }

    // Insert
    if let Ok(collection) = command.get_str("insert") {
        let documents = if let Ok(docs) = command.get_array("documents") {
            docs.iter()
                .filter_map(|d| d.as_document().cloned())
                .collect()
        } else {
            doc_sequence
        };

        match store.insert_many(db, collection, documents).await {
            Ok(ids) => {
                doc! {
                    "n": ids.len() as i32,
                    "ok": 1.0,
                }
            }
            Err(e) => {
                doc! {
                    "n": 0,
                    "ok": 0.0,
                    "errmsg": e,
                    "code": 11000, // Duplicate key error
                }
            }
        }
    }
    // Find
    else if let Ok(collection) = command.get_str("find") {
        let filter = command
            .get_document("filter")
            .cloned()
            .unwrap_or_default();
        let limit = command.get_i64("limit").or_else(|_| command.get_i32("limit").map(|v| v as i64)).ok();
        let skip = command.get_i64("skip").or_else(|_| command.get_i32("skip").map(|v| v as i64)).ok();
        let batch_size = command.get_i32("batchSize").ok();

        let (cursor_id, docs) = store
            .find(db, collection, &filter, limit, skip, batch_size)
            .await;

        doc! {
            "cursor": {
                "id": cursor_id,
                "ns": format!("{}.{}", db, collection),
                "firstBatch": docs.into_iter().map(Bson::Document).collect::<Vec<_>>(),
            },
            "ok": 1.0,
        }
    }
    // FindOne (findOne is typically just find with limit 1)
    else if let Ok(collection) = command.get_str("findOne") {
        let filter = command
            .get_document("filter")
            .cloned()
            .unwrap_or_default();

        if let Some(doc) = store.find_one(db, collection, &filter).await {
            doc! {
                "cursor": {
                    "id": 0i64,
                    "ns": format!("{}.{}", db, collection),
                    "firstBatch": [doc],
                },
                "ok": 1.0,
            }
        } else {
            doc! {
                "cursor": {
                    "id": 0i64,
                    "ns": format!("{}.{}", db, collection),
                    "firstBatch": [],
                },
                "ok": 1.0,
            }
        }
    }
    // Update
    else if let Ok(collection) = command.get_str("update") {
        let updates = command.get_array("updates").ok();
        let mut total_matched = 0i64;
        let mut total_modified = 0i64;

        if let Some(updates) = updates {
            for update in updates {
                if let Some(update_doc) = update.as_document() {
                    let filter = update_doc
                        .get_document("q")
                        .cloned()
                        .unwrap_or_default();
                    let update_spec = update_doc
                        .get_document("u")
                        .cloned()
                        .unwrap_or_default();
                    let multi = update_doc.get_bool("multi").unwrap_or(false);

                    let (matched, modified) = if multi {
                        store.update_many(db, collection, &filter, &update_spec).await
                    } else {
                        store.update_one(db, collection, &filter, &update_spec).await
                    };

                    total_matched += matched;
                    total_modified += modified;
                }
            }
        }

        doc! {
            "n": total_matched as i32,
            "nModified": total_modified as i32,
            "ok": 1.0,
        }
    }
    // Delete
    else if let Ok(collection) = command.get_str("delete") {
        let deletes = command.get_array("deletes").ok();
        let mut total_deleted = 0i64;

        if let Some(deletes) = deletes {
            for delete in deletes {
                if let Some(delete_doc) = delete.as_document() {
                    let filter = delete_doc
                        .get_document("q")
                        .cloned()
                        .unwrap_or_default();
                    let limit = delete_doc.get_i32("limit").unwrap_or(0);

                    let deleted = if limit == 1 {
                        store.delete_one(db, collection, &filter).await
                    } else {
                        store.delete_many(db, collection, &filter).await
                    };

                    total_deleted += deleted;
                }
            }
        }

        doc! {
            "n": total_deleted as i32,
            "ok": 1.0,
        }
    }
    // Count
    else if let Ok(collection) = command.get_str("count") {
        let filter = command
            .get_document("query")
            .or_else(|_| command.get_document("filter"))
            .cloned()
            .unwrap_or_default();

        let count = store.count(db, collection, &filter).await;

        doc! {
            "n": count,
            "ok": 1.0,
        }
    }
    // CountDocuments (newer API)
    else if let Ok(collection) = command.get_str("countDocuments") {
        let filter = command
            .get_document("filter")
            .cloned()
            .unwrap_or_default();

        let count = store.count(db, collection, &filter).await;

        doc! {
            "n": count,
            "ok": 1.0,
        }
    }
    // GetMore (cursor continuation)
    else if command.contains_key("getMore") {
        let cursor_id = command.get_i64("getMore").unwrap_or(0);
        let collection = command.get_str("collection").unwrap_or("unknown");
        let batch_size = command.get_i32("batchSize").ok();

        if let Some((new_cursor_id, docs)) = store.get_more(cursor_id, batch_size).await {
            doc! {
                "cursor": {
                    "id": new_cursor_id,
                    "ns": format!("{}.{}", db, collection),
                    "nextBatch": docs.into_iter().map(Bson::Document).collect::<Vec<_>>(),
                },
                "ok": 1.0,
            }
        } else {
            doc! {
                "ok": 0.0,
                "errmsg": "cursor not found",
                "code": 43,
            }
        }
    }
    // Aggregate (basic support)
    else if let Ok(collection) = command.get_str("aggregate") {
        let pipeline = command.get_array("pipeline").ok();
        let cursor_batch_size = command
            .get_document("cursor")
            .ok()
            .and_then(|c| c.get_i32("batchSize").ok());

        // For now, only support simple pipelines
        let mut docs = Vec::new();
        let mut filter = doc! {};

        if let Some(stages) = pipeline {
            for stage in stages {
                if let Some(stage_doc) = stage.as_document() {
                    // $match stage
                    if let Ok(match_doc) = stage_doc.get_document("$match") {
                        filter = match_doc.clone();
                    }
                    // $limit stage
                    else if let Ok(limit) = stage_doc.get_i64("$limit") {
                        let (_, found) = store
                            .find(db, collection, &filter, Some(limit), None, None)
                            .await;
                        docs = found;
                    }
                    // $count stage
                    else if let Ok(count_field) = stage_doc.get_str("$count") {
                        let count = store.count(db, collection, &filter).await;
                        docs = vec![doc! { count_field: count }];
                    }
                    // Other stages: just pass through for now
                }
            }
        }

        // If no special stage was processed, just run find
        if docs.is_empty() {
            let (_, found) = store.find(db, collection, &filter, None, None, cursor_batch_size).await;
            docs = found;
        }

        doc! {
            "cursor": {
                "id": 0i64,
                "ns": format!("{}.{}", db, collection),
                "firstBatch": docs.into_iter().map(Bson::Document).collect::<Vec<_>>(),
            },
            "ok": 1.0,
        }
    }
    // CreateIndexes
    else if let Ok(collection) = command.get_str("createIndexes") {
        let indexes = command.get_array("indexes").ok();
        let mut created_names = Vec::new();

        if let Some(indexes) = indexes {
            for index in indexes {
                if let Some(index_doc) = index.as_document() {
                    let keys = index_doc.get_document("key").cloned().unwrap_or_default();
                    let name = index_doc.get_str("name").ok().map(|s| s.to_string());
                    let unique = index_doc.get_bool("unique").unwrap_or(false);
                    let sparse = index_doc.get_bool("sparse").unwrap_or(false);

                    let created_name = store
                        .create_index(db, collection, keys, name, unique, sparse)
                        .await;
                    created_names.push(created_name);
                }
            }
        }

        doc! {
            "createdCollectionAutomatically": false,
            "numIndexesBefore": 1,
            "numIndexesAfter": 1 + created_names.len() as i32,
            "ok": 1.0,
        }
    }
    // ListIndexes
    else if let Ok(collection) = command.get_str("listIndexes") {
        let indexes = store.list_indexes(db, collection).await;

        doc! {
            "cursor": {
                "id": 0i64,
                "ns": format!("{}.{}", db, collection),
                "firstBatch": indexes.into_iter().map(Bson::Document).collect::<Vec<_>>(),
            },
            "ok": 1.0,
        }
    }
    // ListCollections
    else if command.contains_key("listCollections") {
        let collections = store.list_collections(db).await;

        doc! {
            "cursor": {
                "id": 0i64,
                "ns": format!("{}.$cmd.listCollections", db),
                "firstBatch": collections.into_iter().map(Bson::Document).collect::<Vec<_>>(),
            },
            "ok": 1.0,
        }
    }
    // ListDatabases
    else if command.contains_key("listDatabases") {
        let databases = store.list_databases().await;
        let total_size: i64 = databases.iter().filter_map(|d| d.get_i64("sizeOnDisk").ok()).sum();

        doc! {
            "databases": databases.into_iter().map(Bson::Document).collect::<Vec<_>>(),
            "totalSize": total_size,
            "ok": 1.0,
        }
    }
    // Drop collection
    else if let Ok(collection) = command.get_str("drop") {
        let dropped = store.drop_collection(db, collection).await;

        if dropped {
            doc! { "ok": 1.0, "ns": format!("{}.{}", db, collection) }
        } else {
            doc! { "ok": 0.0, "errmsg": "ns not found" }
        }
    }
    // DropDatabase
    else if command.contains_key("dropDatabase") {
        store.drop_database(db).await;
        doc! { "ok": 1.0, "dropped": db }
    }
    // Create collection (implicit in MongoDB)
    else if let Ok(collection) = command.get_str("create") {
        // Collections are created implicitly on first insert
        // But we can force creation by inserting and deleting
        store.insert_one(db, collection, doc! { "_orbit_init": true }).await.ok();
        store.delete_one(db, collection, &doc! { "_orbit_init": true }).await;

        doc! { "ok": 1.0 }
    }
    // Ping
    else if command.contains_key("ping") {
        doc! { "ok": 1.0 }
    }
    // BuildInfo
    else if command.contains_key("buildInfo") {
        build_info_response()
    }
    // ServerStatus
    else if command.contains_key("serverStatus") {
        server_status_response()
    }
    // HostInfo
    else if command.contains_key("hostInfo") {
        host_info_response()
    }
    // WhatsmyUri
    else if command.contains_key("whatsmyuri") {
        doc! { "you": "127.0.0.1:27017", "ok": 1.0 }
    }
    // GetLog
    else if command.contains_key("getLog") {
        doc! {
            "log": Bson::Array(vec![]),
            "totalLinesWritten": 0,
            "ok": 1.0,
        }
    }
    // ReplSetGetStatus
    else if command.contains_key("replSetGetStatus") {
        doc! {
            "ok": 0.0,
            "errmsg": "not running with --replSet",
            "code": 76,
        }
    }
    // GetCmdLineOpts
    else if command.contains_key("getCmdLineOpts") {
        doc! {
            "argv": ["orbit-server"],
            "parsed": {},
            "ok": 1.0,
        }
    }
    // GetParameter
    else if command.contains_key("getParameter") {
        doc! { "ok": 1.0 }
    }
    // CollStats
    else if let Ok(collection) = command.get_str("collStats") {
        let count = store.count(db, collection, &doc! {}).await;

        doc! {
            "ns": format!("{}.{}", db, collection),
            "count": count,
            "size": count * 1024,
            "avgObjSize": 1024,
            "storageSize": count * 1024,
            "nindexes": 1,
            "ok": 1.0,
        }
    }
    // DbStats
    else if command.contains_key("dbStats") || command.contains_key("dbstats") {
        let collections = store.list_collections(db).await;

        doc! {
            "db": db,
            "collections": collections.len() as i32,
            "objects": 0,
            "avgObjSize": 0,
            "dataSize": 0,
            "storageSize": 0,
            "indexes": collections.len() as i32,
            "indexSize": 0,
            "ok": 1.0,
        }
    }
    // Unknown command
    else {
        let cmd_name = command
            .keys()
            .next()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        warn!("Unknown MongoDB command: {}", cmd_name);

        doc! {
            "ok": 0.0,
            "errmsg": format!("no such command: '{}'", cmd_name),
            "code": 59,
        }
    }
}

fn make_reply(request_id: i32, documents: Vec<Document>) -> MongoMessage {
    let num = documents.len() as i32;
    MongoMessage::Reply {
        header: MongoHeader {
            message_length: 0,
            request_id: request_id + 1,
            response_to: request_id,
            op_code: OP_REPLY,
        },
        response_flags: 0,
        cursor_id: 0,
        starting_from: 0,
        number_returned: num,
        documents,
    }
}

fn handshake_response() -> Document {
    doc! {
        "ismaster": true,
        "maxBsonObjectSize": 16777216,
        "maxMessageSizeBytes": 48000000,
        "maxWriteBatchSize": 100000,
        "localTime": bson::DateTime::now(),
        "logicalSessionTimeoutMinutes": 30,
        "connectionId": 1,
        "minWireVersion": 0,
        "maxWireVersion": 17, // MongoDB 6.0+
        "readOnly": false,
        "ok": 1.0,
    }
}

fn build_info_response() -> Document {
    doc! {
        "version": "6.0.0",
        "gitVersion": "orbit-rs",
        "modules": Bson::Array(vec![]),
        "allocator": "system",
        "javascriptEngine": "none",
        "sysInfo": "Orbit-RS MongoDB Protocol Adapter",
        "versionArray": [6, 0, 0, 0],
        "bits": 64,
        "debug": false,
        "maxBsonObjectSize": 16777216,
        "storageEngines": ["orbit"],
        "ok": 1.0,
    }
}

fn server_status_response() -> Document {
    doc! {
        "host": "localhost",
        "version": "6.0.0",
        "process": "orbit-server",
        "pid": std::process::id() as i64,
        "uptime": 1000.0,
        "uptimeMillis": 1000000i64,
        "uptimeEstimate": 1000i64,
        "localTime": bson::DateTime::now(),
        "connections": {
            "current": 1,
            "available": 1000,
            "totalCreated": 1,
        },
        "network": {
            "bytesIn": 0i64,
            "bytesOut": 0i64,
            "numRequests": 0i64,
        },
        "ok": 1.0,
    }
}

fn host_info_response() -> Document {
    doc! {
        "system": {
            "currentTime": bson::DateTime::now(),
            "hostname": "localhost",
            "cpuAddrSize": 64,
            "memSizeMB": 8192,
            "numCores": 4,
            "cpuArch": "x86_64",
        },
        "os": {
            "type": std::env::consts::OS,
            "name": std::env::consts::OS,
            "version": "Unknown",
        },
        "ok": 1.0,
    }
}
