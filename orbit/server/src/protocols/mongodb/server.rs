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

            let db = command_doc.get_str("$db").unwrap_or("test").to_string();

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
        let filter = command.get_document("filter").cloned().unwrap_or_default();
        let limit = command
            .get_i64("limit")
            .or_else(|_| command.get_i32("limit").map(|v| v as i64))
            .ok();
        let skip = command
            .get_i64("skip")
            .or_else(|_| command.get_i32("skip").map(|v| v as i64))
            .ok();
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
        let filter = command.get_document("filter").cloned().unwrap_or_default();

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
                    let filter = update_doc.get_document("q").cloned().unwrap_or_default();
                    let update_spec = update_doc.get_document("u").cloned().unwrap_or_default();
                    let multi = update_doc.get_bool("multi").unwrap_or(false);

                    let (matched, modified) = if multi {
                        store
                            .update_many(db, collection, &filter, &update_spec)
                            .await
                    } else {
                        store
                            .update_one(db, collection, &filter, &update_spec)
                            .await
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
                    let filter = delete_doc.get_document("q").cloned().unwrap_or_default();
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
        let filter = command.get_document("filter").cloned().unwrap_or_default();

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
    // Aggregate (enhanced pipeline support)
    else if let Ok(collection) = command.get_str("aggregate") {
        let pipeline = command.get_array("pipeline").ok();
        let _cursor_batch_size = command
            .get_document("cursor")
            .ok()
            .and_then(|c| c.get_i32("batchSize").ok());

        // Start with all documents from collection
        let (_, mut docs) = store.find(db, collection, &doc! {}, None, None, None).await;

        if let Some(stages) = pipeline {
            for stage in stages {
                if let Some(stage_doc) = stage.as_document() {
                    // $match - filter documents
                    if let Ok(match_doc) = stage_doc.get_document("$match") {
                        docs = docs
                            .into_iter()
                            .filter(|d| matches_document_filter(d, match_doc))
                            .collect();
                    }
                    // $project - reshape documents
                    else if let Ok(project_doc) = stage_doc.get_document("$project") {
                        docs = docs
                            .into_iter()
                            .map(|d| apply_projection(&d, project_doc))
                            .collect();
                    }
                    // $sort - order documents
                    else if let Ok(sort_doc) = stage_doc.get_document("$sort") {
                        docs = apply_sort(docs, sort_doc);
                    }
                    // $limit - limit result count
                    else if let Ok(limit) = stage_doc.get_i64("$limit") {
                        docs = docs.into_iter().take(limit as usize).collect();
                    } else if let Ok(limit) = stage_doc.get_i32("$limit") {
                        docs = docs.into_iter().take(limit as usize).collect();
                    }
                    // $skip - skip documents
                    else if let Ok(skip) = stage_doc.get_i64("$skip") {
                        docs = docs.into_iter().skip(skip as usize).collect();
                    } else if let Ok(skip) = stage_doc.get_i32("$skip") {
                        docs = docs.into_iter().skip(skip as usize).collect();
                    }
                    // $count - count documents
                    else if let Ok(count_field) = stage_doc.get_str("$count") {
                        let count = docs.len() as i64;
                        docs = vec![doc! { count_field: count }];
                    }
                    // $group - group and aggregate
                    else if let Ok(group_doc) = stage_doc.get_document("$group") {
                        docs = apply_group(&docs, group_doc);
                    }
                    // $unwind - flatten arrays
                    else if let Some(unwind_spec) = stage_doc.get("$unwind") {
                        docs = apply_unwind(docs, unwind_spec);
                    }
                    // $addFields - add computed fields
                    else if let Ok(add_fields_doc) = stage_doc.get_document("$addFields") {
                        docs = docs
                            .into_iter()
                            .map(|mut d| {
                                for (key, value) in add_fields_doc {
                                    let computed = evaluate_expression(value, &d);
                                    d.insert(key, computed);
                                }
                                d
                            })
                            .collect();
                    }
                    // $replaceRoot - replace document with subdocument
                    else if let Ok(replace_doc) = stage_doc.get_document("$replaceRoot") {
                        if let Some(new_root) = replace_doc.get("newRoot") {
                            docs = docs
                                .into_iter()
                                .filter_map(|d| {
                                    let result = evaluate_expression(new_root, &d);
                                    if let Bson::Document(new_doc) = result {
                                        Some(new_doc)
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                        }
                    }
                    // $lookup - join with another collection
                    else if let Ok(lookup_doc) = stage_doc.get_document("$lookup") {
                        let from_collection = lookup_doc.get_str("from").unwrap_or("");
                        let local_field = lookup_doc.get_str("localField").unwrap_or("");
                        let foreign_field = lookup_doc.get_str("foreignField").unwrap_or("");
                        let as_field = lookup_doc.get_str("as").unwrap_or("result");

                        // Get documents from the foreign collection
                        let (_, foreign_docs) = store
                            .find(db, from_collection, &doc! {}, None, None, None)
                            .await;

                        docs = docs
                            .into_iter()
                            .map(|mut d| {
                                let local_val = d.get(local_field).cloned();
                                let matching: Vec<Bson> = foreign_docs
                                    .iter()
                                    .filter(|fd| {
                                        if let Some(local) = &local_val {
                                            fd.get(foreign_field) == Some(local)
                                        } else {
                                            false
                                        }
                                    })
                                    .map(|fd| Bson::Document(fd.clone()))
                                    .collect();
                                d.insert(as_field, Bson::Array(matching));
                                d
                            })
                            .collect();
                    }
                    // $out - write to collection (final stage)
                    else if let Ok(out_collection) = stage_doc.get_str("$out") {
                        // Drop and recreate collection with results
                        store.drop_collection(db, out_collection).await;
                        for d in &docs {
                            let _ = store.insert_one(db, out_collection, d.clone()).await;
                        }
                    }
                    // $sample - random sample
                    else if let Ok(sample_doc) = stage_doc.get_document("$sample") {
                        if let Ok(size) = sample_doc.get_i64("size") {
                            use std::collections::HashSet;
                            if (size as usize) < docs.len() {
                                let mut indices = HashSet::new();
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_nanos() as usize;
                                let mut seed = now;
                                while indices.len() < size as usize {
                                    seed = (seed * 1103515245 + 12345) % (1 << 31);
                                    indices.insert(seed % docs.len());
                                }
                                docs = indices
                                    .into_iter()
                                    .filter_map(|i| docs.get(i).cloned())
                                    .collect();
                            }
                        }
                    }
                }
            }
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
        let total_size: i64 = databases
            .iter()
            .filter_map(|d| d.get_i64("sizeOnDisk").ok())
            .sum();

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
        store
            .insert_one(db, collection, doc! { "_orbit_init": true })
            .await
            .ok();
        store
            .delete_one(db, collection, &doc! { "_orbit_init": true })
            .await;

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

// ============================================================================
// Aggregation Pipeline Helper Functions
// ============================================================================

/// Check if a document matches a filter (for $match stage)
fn matches_document_filter(doc: &Document, filter: &Document) -> bool {
    use super::storage::matches_filter;
    matches_filter(doc, filter)
}

/// Apply $project stage to reshape a document
fn apply_projection(doc: &Document, project: &Document) -> Document {
    let mut result = Document::new();

    for (key, value) in project {
        match value {
            // Include field: { field: 1 }
            Bson::Int32(1) | Bson::Int64(1) | Bson::Boolean(true) => {
                if let Some(v) = doc.get(key) {
                    result.insert(key, v.clone());
                }
            }
            // Exclude field: { field: 0 } - skip
            Bson::Int32(0) | Bson::Int64(0) | Bson::Boolean(false) => {
                // Don't include this field
            }
            // Rename/compute: { newField: "$oldField" }
            Bson::String(s) if s.starts_with('$') => {
                let field_name = &s[1..];
                if let Some(v) = get_nested_field(doc, field_name) {
                    result.insert(key, v);
                }
            }
            // Expression: { field: { $expr: ... } }
            Bson::Document(expr_doc) => {
                let computed = evaluate_expression(&Bson::Document(expr_doc.clone()), doc);
                result.insert(key, computed);
            }
            // Literal value
            other => {
                result.insert(key, other.clone());
            }
        }
    }

    // If no explicit inclusions, include all except explicitly excluded
    let has_inclusions = project
        .values()
        .any(|v| matches!(v, Bson::Int32(1) | Bson::Int64(1) | Bson::Boolean(true)));
    if !has_inclusions && result.is_empty() {
        for (key, value) in doc {
            let should_exclude = project.get(key).map_or(false, |v| {
                matches!(v, Bson::Int32(0) | Bson::Int64(0) | Bson::Boolean(false))
            });
            if !should_exclude {
                result.insert(key, value.clone());
            }
        }
    }

    // Always include _id unless explicitly excluded
    if project.get("_id").map_or(true, |v| {
        !matches!(v, Bson::Int32(0) | Bson::Int64(0) | Bson::Boolean(false))
    }) {
        if let Some(id) = doc.get("_id") {
            result.insert("_id", id.clone());
        }
    }

    result
}

/// Get a nested field value using dot notation
fn get_nested_field(doc: &Document, path: &str) -> Option<Bson> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = Bson::Document(doc.clone());

    for part in parts {
        match &current {
            Bson::Document(d) => {
                current = d.get(part)?.clone();
            }
            Bson::Array(arr) => {
                if let Ok(idx) = part.parse::<usize>() {
                    current = arr.get(idx)?.clone();
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }

    Some(current)
}

/// Apply $sort stage
fn apply_sort(mut docs: Vec<Document>, sort_doc: &Document) -> Vec<Document> {
    docs.sort_by(|a, b| {
        for (key, order) in sort_doc {
            let order_val = match order {
                Bson::Int32(n) => *n,
                Bson::Int64(n) => *n as i32,
                _ => 1,
            };

            let a_val = a.get(key);
            let b_val = b.get(key);

            let cmp = compare_bson_values(a_val, b_val);
            if cmp != std::cmp::Ordering::Equal {
                return if order_val < 0 { cmp.reverse() } else { cmp };
            }
        }
        std::cmp::Ordering::Equal
    });
    docs
}

/// Compare two BSON values for sorting
fn compare_bson_values(a: Option<&Bson>, b: Option<&Bson>) -> std::cmp::Ordering {
    match (a, b) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(Bson::Null), Some(Bson::Null)) => std::cmp::Ordering::Equal,
        (Some(Bson::Null), _) => std::cmp::Ordering::Less,
        (_, Some(Bson::Null)) => std::cmp::Ordering::Greater,
        (Some(Bson::Int32(x)), Some(Bson::Int32(y))) => x.cmp(y),
        (Some(Bson::Int64(x)), Some(Bson::Int64(y))) => x.cmp(y),
        (Some(Bson::Int32(x)), Some(Bson::Int64(y))) => (*x as i64).cmp(y),
        (Some(Bson::Int64(x)), Some(Bson::Int32(y))) => x.cmp(&(*y as i64)),
        (Some(Bson::Double(x)), Some(Bson::Double(y))) => {
            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Some(Bson::String(x)), Some(Bson::String(y))) => x.cmp(y),
        (Some(Bson::Boolean(x)), Some(Bson::Boolean(y))) => x.cmp(y),
        (Some(Bson::DateTime(x)), Some(Bson::DateTime(y))) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    }
}

/// Apply $group stage
fn apply_group(docs: &[Document], group_doc: &Document) -> Vec<Document> {
    use std::collections::HashMap;

    let id_expr = group_doc.get("_id");

    // Group documents by _id expression
    let mut groups: HashMap<String, (Bson, Vec<&Document>)> = HashMap::new();

    for doc in docs {
        let group_key = if let Some(id) = id_expr {
            evaluate_expression(id, doc)
        } else {
            Bson::Null
        };
        let key_str = format!("{:?}", group_key);
        groups
            .entry(key_str)
            .or_insert_with(|| (group_key.clone(), Vec::new()))
            .1
            .push(doc);
    }

    // Apply accumulators to each group
    groups
        .into_values()
        .map(|(group_id, group_docs)| {
            let mut result = doc! { "_id": group_id };

            for (key, value) in group_doc {
                if key == "_id" {
                    continue;
                }

                if let Bson::Document(acc_doc) = value {
                    if let Some((acc_op, acc_expr)) = acc_doc.iter().next() {
                        let accumulated = apply_accumulator(acc_op, acc_expr, &group_docs);
                        result.insert(key, accumulated);
                    }
                }
            }

            result
        })
        .collect()
}

/// Apply an accumulator operator
fn apply_accumulator(op: &str, expr: &Bson, docs: &[&Document]) -> Bson {
    match op {
        "$sum" => {
            let mut sum = 0.0f64;
            for doc in docs {
                if let Some(val) = bson_to_f64(&evaluate_expression(expr, doc)) {
                    sum += val;
                }
            }
            if sum.fract() == 0.0 {
                Bson::Int64(sum as i64)
            } else {
                Bson::Double(sum)
            }
        }
        "$avg" => {
            let mut sum = 0.0f64;
            let mut count = 0;
            for doc in docs {
                if let Some(val) = bson_to_f64(&evaluate_expression(expr, doc)) {
                    sum += val;
                    count += 1;
                }
            }
            if count > 0 {
                Bson::Double(sum / count as f64)
            } else {
                Bson::Null
            }
        }
        "$min" => {
            let mut min: Option<Bson> = None;
            for doc in docs {
                let val = evaluate_expression(expr, doc);
                if min.is_none()
                    || compare_bson_values(Some(&val), min.as_ref()) == std::cmp::Ordering::Less
                {
                    min = Some(val);
                }
            }
            min.unwrap_or(Bson::Null)
        }
        "$max" => {
            let mut max: Option<Bson> = None;
            for doc in docs {
                let val = evaluate_expression(expr, doc);
                if max.is_none()
                    || compare_bson_values(Some(&val), max.as_ref()) == std::cmp::Ordering::Greater
                {
                    max = Some(val);
                }
            }
            max.unwrap_or(Bson::Null)
        }
        "$count" => Bson::Int64(docs.len() as i64),
        "$first" => docs
            .first()
            .map(|d| evaluate_expression(expr, d))
            .unwrap_or(Bson::Null),
        "$last" => docs
            .last()
            .map(|d| evaluate_expression(expr, d))
            .unwrap_or(Bson::Null),
        "$push" => {
            let values: Vec<Bson> = docs.iter().map(|d| evaluate_expression(expr, d)).collect();
            Bson::Array(values)
        }
        "$addToSet" => {
            let mut seen = std::collections::HashSet::new();
            let mut values = Vec::new();
            for doc in docs {
                let val = evaluate_expression(expr, doc);
                let key = format!("{:?}", val);
                if seen.insert(key) {
                    values.push(val);
                }
            }
            Bson::Array(values)
        }
        _ => Bson::Null,
    }
}

/// Convert BSON to f64 for numeric operations
fn bson_to_f64(bson: &Bson) -> Option<f64> {
    match bson {
        Bson::Int32(n) => Some(*n as f64),
        Bson::Int64(n) => Some(*n as f64),
        Bson::Double(n) => Some(*n),
        _ => None,
    }
}

/// Apply $unwind stage
fn apply_unwind(docs: Vec<Document>, unwind_spec: &Bson) -> Vec<Document> {
    let (path, preserve_null) = match unwind_spec {
        Bson::String(s) => (s.trim_start_matches('$').to_string(), false),
        Bson::Document(d) => {
            let path = d
                .get_str("path")
                .unwrap_or("")
                .trim_start_matches('$')
                .to_string();
            let preserve = d.get_bool("preserveNullAndEmptyArrays").unwrap_or(false);
            (path, preserve)
        }
        _ => return docs,
    };

    let mut result = Vec::new();

    for doc in docs {
        if let Some(Bson::Array(arr)) = doc.get(&path) {
            if arr.is_empty() {
                if preserve_null {
                    let mut new_doc = doc.clone();
                    new_doc.remove(&path);
                    result.push(new_doc);
                }
            } else {
                for item in arr {
                    let mut new_doc = doc.clone();
                    new_doc.insert(&path, item.clone());
                    result.push(new_doc);
                }
            }
        } else if preserve_null {
            result.push(doc);
        }
    }

    result
}

/// Evaluate an aggregation expression
fn evaluate_expression(expr: &Bson, doc: &Document) -> Bson {
    match expr {
        // Field reference: "$fieldName"
        Bson::String(s) if s.starts_with('$') => {
            let field_path = &s[1..];
            get_nested_field(doc, field_path).unwrap_or(Bson::Null)
        }
        // Expression document
        Bson::Document(expr_doc) => {
            if let Some((op, args)) = expr_doc.iter().next() {
                match op.as_str() {
                    // Arithmetic
                    "$add" => {
                        if let Bson::Array(arr) = args {
                            let sum: f64 = arr
                                .iter()
                                .filter_map(|v| bson_to_f64(&evaluate_expression(v, doc)))
                                .sum();
                            Bson::Double(sum)
                        } else {
                            Bson::Null
                        }
                    }
                    "$subtract" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let a =
                                    bson_to_f64(&evaluate_expression(&arr[0], doc)).unwrap_or(0.0);
                                let b =
                                    bson_to_f64(&evaluate_expression(&arr[1], doc)).unwrap_or(0.0);
                                return Bson::Double(a - b);
                            }
                        }
                        Bson::Null
                    }
                    "$multiply" => {
                        if let Bson::Array(arr) = args {
                            let product: f64 = arr
                                .iter()
                                .filter_map(|v| bson_to_f64(&evaluate_expression(v, doc)))
                                .product();
                            Bson::Double(product)
                        } else {
                            Bson::Null
                        }
                    }
                    "$divide" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let a =
                                    bson_to_f64(&evaluate_expression(&arr[0], doc)).unwrap_or(0.0);
                                let b =
                                    bson_to_f64(&evaluate_expression(&arr[1], doc)).unwrap_or(1.0);
                                if b != 0.0 {
                                    return Bson::Double(a / b);
                                }
                            }
                        }
                        Bson::Null
                    }
                    // Comparison
                    "$eq" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let a = evaluate_expression(&arr[0], doc);
                                let b = evaluate_expression(&arr[1], doc);
                                return Bson::Boolean(a == b);
                            }
                        }
                        Bson::Boolean(false)
                    }
                    "$ne" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let a = evaluate_expression(&arr[0], doc);
                                let b = evaluate_expression(&arr[1], doc);
                                return Bson::Boolean(a != b);
                            }
                        }
                        Bson::Boolean(false)
                    }
                    "$gt" | "$gte" | "$lt" | "$lte" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let a = evaluate_expression(&arr[0], doc);
                                let b = evaluate_expression(&arr[1], doc);
                                let cmp = compare_bson_values(Some(&a), Some(&b));
                                let result = match op.as_str() {
                                    "$gt" => cmp == std::cmp::Ordering::Greater,
                                    "$gte" => cmp != std::cmp::Ordering::Less,
                                    "$lt" => cmp == std::cmp::Ordering::Less,
                                    "$lte" => cmp != std::cmp::Ordering::Greater,
                                    _ => false,
                                };
                                return Bson::Boolean(result);
                            }
                        }
                        Bson::Boolean(false)
                    }
                    // Conditional
                    "$cond" => {
                        if let Bson::Document(cond_doc) = args {
                            let if_expr = cond_doc.get("if").unwrap_or(&Bson::Boolean(false));
                            let then_expr = cond_doc.get("then").unwrap_or(&Bson::Null);
                            let else_expr = cond_doc.get("else").unwrap_or(&Bson::Null);

                            let condition = match evaluate_expression(if_expr, doc) {
                                Bson::Boolean(b) => b,
                                Bson::Null => false,
                                _ => true,
                            };

                            if condition {
                                evaluate_expression(then_expr, doc)
                            } else {
                                evaluate_expression(else_expr, doc)
                            }
                        } else if let Bson::Array(arr) = args {
                            if arr.len() == 3 {
                                let condition = match evaluate_expression(&arr[0], doc) {
                                    Bson::Boolean(b) => b,
                                    Bson::Null => false,
                                    _ => true,
                                };
                                if condition {
                                    return evaluate_expression(&arr[1], doc);
                                } else {
                                    return evaluate_expression(&arr[2], doc);
                                }
                            }
                            Bson::Null
                        } else {
                            Bson::Null
                        }
                    }
                    "$ifNull" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let val = evaluate_expression(&arr[0], doc);
                                if matches!(val, Bson::Null) {
                                    return evaluate_expression(&arr[1], doc);
                                }
                                return val;
                            }
                        }
                        Bson::Null
                    }
                    // String operations
                    "$concat" => {
                        if let Bson::Array(arr) = args {
                            let parts: Vec<String> = arr
                                .iter()
                                .map(|v| match evaluate_expression(v, doc) {
                                    Bson::String(s) => s,
                                    other => format!("{:?}", other),
                                })
                                .collect();
                            Bson::String(parts.join(""))
                        } else {
                            Bson::Null
                        }
                    }
                    "$toUpper" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::String(s) = val {
                            Bson::String(s.to_uppercase())
                        } else {
                            Bson::Null
                        }
                    }
                    "$toLower" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::String(s) = val {
                            Bson::String(s.to_lowercase())
                        } else {
                            Bson::Null
                        }
                    }
                    // Array operations
                    "$size" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::Array(arr) = val {
                            Bson::Int32(arr.len() as i32)
                        } else {
                            Bson::Null
                        }
                    }
                    "$arrayElemAt" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let array = evaluate_expression(&arr[0], doc);
                                let idx = evaluate_expression(&arr[1], doc);
                                if let (Bson::Array(arr), Bson::Int32(i)) = (array, idx) {
                                    let index =
                                        if i < 0 { arr.len() as i32 + i } else { i } as usize;
                                    return arr.get(index).cloned().unwrap_or(Bson::Null);
                                }
                            }
                        }
                        Bson::Null
                    }
                    // Type conversion
                    "$toString" => {
                        let val = evaluate_expression(args, doc);
                        Bson::String(format!("{:?}", val))
                    }
                    "$toInt" => {
                        let val = evaluate_expression(args, doc);
                        match val {
                            Bson::Int32(n) => Bson::Int32(n),
                            Bson::Int64(n) => Bson::Int32(n as i32),
                            Bson::Double(n) => Bson::Int32(n as i32),
                            Bson::String(s) => {
                                s.parse::<i32>().map(Bson::Int32).unwrap_or(Bson::Null)
                            }
                            _ => Bson::Null,
                        }
                    }
                    "$literal" => args.clone(),
                    _ => Bson::Null,
                }
            } else {
                Bson::Document(expr_doc.clone())
            }
        }
        // Literal value
        other => other.clone(),
    }
}
