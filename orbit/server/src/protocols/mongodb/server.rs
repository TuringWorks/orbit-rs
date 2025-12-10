//! MongoDB protocol server implementation
//!
//! Provides a MongoDB-compatible wire protocol server with full CRUD support.

use super::protocol::{MongoCodec, MongoHeader, MongoMessage, MsgSection, OP_MSG, OP_REPLY};
use super::storage::DocumentStore;
use crate::config::TlsConfig;
use crate::protocols::tls::OrbitTlsAcceptor;
use bson::{doc, oid::ObjectId, Bson, Document};
use futures::{SinkExt, StreamExt};
use orbit_shared::OrbitResult;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio_util::codec::Framed;
use tracing::{debug, error, info, warn};

pub struct MongoDbServer {
    address: String,
    store: Arc<DocumentStore>,
    tls_acceptor: Option<OrbitTlsAcceptor>,
}

impl MongoDbServer {
    pub fn new(address: String) -> Self {
        Self {
            address,
            store: Arc::new(DocumentStore::new()),
            tls_acceptor: None,
        }
    }

    pub fn with_store(address: String, store: Arc<DocumentStore>) -> Self {
        Self {
            address,
            store,
            tls_acceptor: None,
        }
    }

    pub fn with_tls_config(mut self, tls_config: Option<TlsConfig>) -> Self {
        if tls_config.is_some() {
            self.tls_acceptor =
                Some(OrbitTlsAcceptor::new(&tls_config).expect("Invalid TLS configuration"));
        }
        self
    }

    pub async fn run(&self) -> OrbitResult<()> {
        let listener = TcpListener::bind(&self.address).await?;
        info!("MongoDB server listening on {}", self.address);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    debug!("New MongoDB connection from {}", addr);
                    let store = self.store.clone();
                    let tls_acceptor = self.tls_acceptor.clone();

                    tokio::spawn(async move {
                        if let Some(acceptor) = tls_acceptor {
                            match acceptor.accept(socket).await {
                                Ok(tls_stream) => {
                                    if let Err(e) = handle_connection(tls_stream, store).await {
                                        error!("MongoDB connection error: {}", e);
                                    }
                                }
                                Err(e) => {
                                    error!("MongoDB TLS handshake failed: {}", e);
                                }
                            }
                        } else {
                            if let Err(e) = handle_connection(socket, store).await {
                                error!("MongoDB connection error: {}", e);
                            }
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

async fn handle_connection<S>(socket: S, store: Arc<DocumentStore>) -> OrbitResult<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
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
                        docs.retain(|d| matches_document_filter(d, match_doc));
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
                    // $set - alias for $addFields
                    else if let Ok(set_doc) = stage_doc.get_document("$set") {
                        docs = docs
                            .into_iter()
                            .map(|mut d| {
                                for (key, value) in set_doc {
                                    let computed = evaluate_expression(value, &d);
                                    d.insert(key, computed);
                                }
                                d
                            })
                            .collect();
                    }
                    // $unset - remove fields from documents
                    else if let Some(unset_value) = stage_doc.get("$unset") {
                        match unset_value {
                            Bson::String(field) => {
                                docs = docs
                                    .into_iter()
                                    .map(|mut d| {
                                        d.remove(field);
                                        d
                                    })
                                    .collect();
                            }
                            Bson::Array(fields) => {
                                docs = docs
                                    .into_iter()
                                    .map(|mut d| {
                                        for field in fields {
                                            if let Bson::String(f) = field {
                                                d.remove(f);
                                            }
                                        }
                                        d
                                    })
                                    .collect();
                            }
                            _ => {}
                        }
                    }
                    // $sortByCount - group by field and count, sorted by count descending
                    else if let Some(sort_by_count_expr) = stage_doc.get("$sortByCount") {
                        use std::collections::HashMap;
                        let mut counts: HashMap<String, (Bson, i64)> = HashMap::new();

                        for doc in &docs {
                            let key = evaluate_expression(sort_by_count_expr, doc);
                            let key_str = format!("{:?}", key);
                            counts
                                .entry(key_str)
                                .and_modify(|(_, count)| *count += 1)
                                .or_insert((key, 1));
                        }

                        let mut result: Vec<Document> = counts
                            .into_values()
                            .map(|(id, count)| doc! { "_id": id, "count": count })
                            .collect();

                        // Sort by count descending
                        result.sort_by(|a, b| {
                            let a_count = a.get_i64("count").unwrap_or(0);
                            let b_count = b.get_i64("count").unwrap_or(0);
                            b_count.cmp(&a_count)
                        });

                        docs = result;
                    }
                    // $bucket - categorize by boundaries
                    else if let Ok(bucket_doc) = stage_doc.get_document("$bucket") {
                        use std::collections::HashMap;

                        let group_by = bucket_doc.get("groupBy");
                        let boundaries = bucket_doc.get_array("boundaries").ok();
                        let default_val = bucket_doc.get("default");
                        let output_spec = bucket_doc.get_document("output").ok();

                        if let (Some(group_by), Some(boundaries)) = (group_by, boundaries) {
                            let mut buckets: HashMap<String, Vec<&Document>> = HashMap::new();

                            for doc in &docs {
                                let value = evaluate_expression(group_by, doc);
                                let value_f64 = match &value {
                                    Bson::Int32(n) => *n as f64,
                                    Bson::Int64(n) => *n as f64,
                                    Bson::Double(n) => *n,
                                    _ => {
                                        if let Some(def) = default_val {
                                            let key = format!("{:?}", def);
                                            buckets.entry(key).or_default().push(doc);
                                        }
                                        continue;
                                    }
                                };

                                let mut bucket_key = None;
                                for i in 0..boundaries.len() - 1 {
                                    let low = match &boundaries[i] {
                                        Bson::Int32(n) => *n as f64,
                                        Bson::Int64(n) => *n as f64,
                                        Bson::Double(n) => *n,
                                        _ => continue,
                                    };
                                    let high = match &boundaries[i + 1] {
                                        Bson::Int32(n) => *n as f64,
                                        Bson::Int64(n) => *n as f64,
                                        Bson::Double(n) => *n,
                                        _ => continue,
                                    };

                                    if value_f64 >= low && value_f64 < high {
                                        bucket_key = Some(boundaries[i].clone());
                                        break;
                                    }
                                }

                                if let Some(key) = bucket_key {
                                    buckets.entry(format!("{:?}", key)).or_default().push(doc);
                                } else if let Some(def) = default_val {
                                    buckets.entry(format!("{:?}", def)).or_default().push(doc);
                                }
                            }

                            docs = buckets
                                .into_iter()
                                .map(|(key_str, bucket_docs)| {
                                    let mut result =
                                        doc! { "_id": key_str, "count": bucket_docs.len() as i64 };
                                    if let Some(output) = output_spec {
                                        for (field, acc) in output {
                                            if let Bson::Document(acc_doc) = acc {
                                                let doc_refs: Vec<&Document> = bucket_docs.to_vec();
                                                if let Some((op, expr)) = acc_doc.iter().next() {
                                                    let value =
                                                        apply_accumulator(op, expr, &doc_refs);
                                                    result.insert(field, value);
                                                }
                                            }
                                        }
                                    }
                                    result
                                })
                                .collect();
                        }
                    }
                    // $facet - run multiple pipelines in parallel
                    else if let Ok(facet_doc) = stage_doc.get_document("$facet") {
                        let mut facet_result = Document::new();

                        for (facet_name, pipeline_arr) in facet_doc {
                            if let Bson::Array(pipeline_stages) = pipeline_arr {
                                let mut facet_docs = docs.clone();

                                // Process each stage in this facet's pipeline
                                for stage in pipeline_stages {
                                    if let Bson::Document(inner_stage) = stage {
                                        // $match
                                        if let Ok(match_doc) = inner_stage.get_document("$match") {
                                            facet_docs
                                                .retain(|d| matches_document_filter(d, match_doc));
                                        }
                                        // $limit
                                        else if let Ok(limit) = inner_stage.get_i64("$limit") {
                                            facet_docs = facet_docs
                                                .into_iter()
                                                .take(limit as usize)
                                                .collect();
                                        } else if let Ok(limit) = inner_stage.get_i32("$limit") {
                                            facet_docs = facet_docs
                                                .into_iter()
                                                .take(limit as usize)
                                                .collect();
                                        }
                                        // $skip
                                        else if let Ok(skip) = inner_stage.get_i64("$skip") {
                                            facet_docs = facet_docs
                                                .into_iter()
                                                .skip(skip as usize)
                                                .collect();
                                        } else if let Ok(skip) = inner_stage.get_i32("$skip") {
                                            facet_docs = facet_docs
                                                .into_iter()
                                                .skip(skip as usize)
                                                .collect();
                                        }
                                        // $sort
                                        else if let Ok(sort_doc) =
                                            inner_stage.get_document("$sort")
                                        {
                                            facet_docs = apply_sort(facet_docs, sort_doc);
                                        }
                                        // $count
                                        else if let Ok(count_field) =
                                            inner_stage.get_str("$count")
                                        {
                                            let count = facet_docs.len() as i64;
                                            facet_docs = vec![doc! { count_field: count }];
                                        }
                                        // $project
                                        else if let Ok(project_doc) =
                                            inner_stage.get_document("$project")
                                        {
                                            facet_docs = facet_docs
                                                .into_iter()
                                                .map(|d| apply_projection(&d, project_doc))
                                                .collect();
                                        }
                                    }
                                }

                                facet_result.insert(
                                    facet_name,
                                    Bson::Array(
                                        facet_docs.into_iter().map(Bson::Document).collect(),
                                    ),
                                );
                            }
                        }

                        docs = vec![facet_result];
                    }
                    // $redact - filter based on document structure
                    else if let Some(redact_expr) = stage_doc.get("$redact") {
                        docs = docs
                            .into_iter()
                            .filter_map(|doc| {
                                let result = evaluate_expression(redact_expr, &doc);
                                match result {
                                    Bson::String(s) if s == "$$DESCEND" || s == "$$KEEP" => {
                                        Some(doc)
                                    }
                                    Bson::String(s) if s == "$$PRUNE" => None,
                                    _ => Some(doc),
                                }
                            })
                            .collect();
                    }
                    // $merge - write results to a collection
                    else if let Ok(merge_doc) = stage_doc.get_document("$merge") {
                        let into_collection = merge_doc.get_str("into").unwrap_or("");
                        let on_fields = merge_doc.get_array("on").ok();
                        let when_matched = merge_doc.get_str("whenMatched").unwrap_or("replace");
                        let when_not_matched =
                            merge_doc.get_str("whenNotMatched").unwrap_or("insert");

                        for doc in &docs {
                            // Build filter from on fields
                            let filter = if let Some(fields) = on_fields {
                                let mut f = Document::new();
                                for field in fields {
                                    if let Bson::String(field_name) = field {
                                        if let Some(val) = doc.get(field_name) {
                                            f.insert(field_name.clone(), val.clone());
                                        }
                                    }
                                }
                                f
                            } else {
                                doc! { "_id": doc.get("_id").cloned().unwrap_or(Bson::Null) }
                            };

                            let existing = store.find_one(db, into_collection, &filter).await;

                            if existing.is_some() {
                                match when_matched {
                                    "replace" => {
                                        store.update_one(db, into_collection, &filter, doc).await;
                                    }
                                    "merge" => {
                                        let update = doc! { "$set": doc.clone() };
                                        store
                                            .update_one(db, into_collection, &filter, &update)
                                            .await;
                                    }
                                    "keepExisting" => {}
                                    "fail" => {
                                        // Would return an error in production
                                    }
                                    _ => {}
                                }
                            } else if when_not_matched == "insert" {
                                let _ = store.insert_one(db, into_collection, doc.clone()).await;
                            }
                        }
                    }
                    // $graphLookup - recursive lookup
                    else if let Ok(graph_doc) = stage_doc.get_document("$graphLookup") {
                        let from = graph_doc.get_str("from").unwrap_or("");
                        let start_with = graph_doc.get("startWith");
                        let connect_from = graph_doc.get_str("connectFromField").unwrap_or("");
                        let connect_to = graph_doc.get_str("connectToField").unwrap_or("");
                        let as_field = graph_doc.get_str("as").unwrap_or("result");
                        let max_depth = graph_doc
                            .get_i32("maxDepth")
                            .ok()
                            .or_else(|| graph_doc.get_i64("maxDepth").ok().map(|v| v as i32));
                        let depth_field = graph_doc.get_str("depthField").ok();

                        // Get all documents from the from collection
                        let (_, from_docs) = store.find(db, from, &doc! {}, None, None, None).await;

                        docs = docs
                            .into_iter()
                            .map(|mut d| {
                                let mut results = Vec::new();
                                let mut visited: std::collections::HashSet<String> =
                                    std::collections::HashSet::new();

                                // Get starting values
                                let start_values = if let Some(start) = start_with {
                                    vec![evaluate_expression(start, &d)]
                                } else {
                                    Vec::new()
                                };

                                // BFS traversal
                                let mut queue: std::collections::VecDeque<(Bson, i32)> =
                                    start_values.into_iter().map(|v| (v, 0)).collect();

                                while let Some((current_val, depth)) = queue.pop_front() {
                                    if let Some(max) = max_depth {
                                        if depth > max {
                                            continue;
                                        }
                                    }

                                    let val_key = format!("{:?}", current_val);
                                    if visited.contains(&val_key) {
                                        continue;
                                    }
                                    visited.insert(val_key);

                                    // Find matching documents
                                    for from_doc in &from_docs {
                                        if let Some(connect_val) = from_doc.get(connect_to) {
                                            if *connect_val == current_val {
                                                let mut result_doc = from_doc.clone();
                                                if let Some(depth_f) = depth_field {
                                                    result_doc.insert(depth_f, depth);
                                                }
                                                results.push(Bson::Document(result_doc.clone()));

                                                // Add to queue for next level
                                                if let Some(next_val) = from_doc.get(connect_from) {
                                                    queue.push_back((next_val.clone(), depth + 1));
                                                }
                                            }
                                        }
                                    }
                                }

                                d.insert(as_field, Bson::Array(results));
                                d
                            })
                            .collect();
                    }
                    // $bucketAuto - automatically determine bucket boundaries
                    else if let Ok(bucket_doc) = stage_doc.get_document("$bucketAuto") {
                        let group_by = bucket_doc.get("groupBy");
                        let buckets_count = bucket_doc
                            .get_i32("buckets")
                            .ok()
                            .or_else(|| bucket_doc.get_i64("buckets").ok().map(|v| v as i32))
                            .unwrap_or(5);
                        let output = bucket_doc.get_document("output").ok();
                        let granularity = bucket_doc.get_str("granularity").ok();

                        if let Some(group_expr) = group_by {
                            // Collect all values for grouping
                            let mut values: Vec<(f64, Document)> = docs
                                .iter()
                                .filter_map(|d| {
                                    let val = evaluate_expression(group_expr, d);
                                    bson_to_f64(&val).map(|v| (v, d.clone()))
                                })
                                .collect();

                            // Sort by value
                            values.sort_by(|a, b| {
                                a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)
                            });

                            if !values.is_empty() {
                                let bucket_size =
                                    (values.len() as f64 / buckets_count as f64).ceil() as usize;
                                let bucket_size = bucket_size.max(1);

                                let mut bucket_results = Vec::new();
                                let mut i = 0;

                                while i < values.len() {
                                    let end = (i + bucket_size).min(values.len());
                                    let bucket_docs: Vec<&Document> =
                                        values[i..end].iter().map(|(_, d)| d).collect();

                                    let min_val = values[i].0;
                                    let max_val = if end < values.len() {
                                        values[end].0
                                    } else {
                                        values[end - 1].0
                                    };

                                    // Apply granularity if specified
                                    let (adjusted_min, adjusted_max) =
                                        if let Some(gran) = granularity {
                                            apply_granularity(min_val, max_val, gran)
                                        } else {
                                            (min_val, max_val)
                                        };

                                    let mut bucket_doc = doc! {
                                        "_id": {
                                            "min": adjusted_min,
                                            "max": adjusted_max
                                        },
                                        "count": bucket_docs.len() as i64
                                    };

                                    // Apply output accumulators
                                    if let Some(output_doc) = &output {
                                        for (field, acc_doc) in output_doc.iter() {
                                            if let Bson::Document(acc) = acc_doc {
                                                if let Some((acc_op, acc_expr)) = acc.iter().next()
                                                {
                                                    let result = apply_accumulator(
                                                        acc_op,
                                                        acc_expr,
                                                        &bucket_docs,
                                                    );
                                                    bucket_doc.insert(field, result);
                                                }
                                            }
                                        }
                                    }

                                    bucket_results.push(bucket_doc);
                                    i = end;
                                }

                                docs = bucket_results;
                            } else {
                                docs = Vec::new();
                            }
                        }
                    }
                    // $unionWith - combine documents from another collection
                    else if let Some(union_spec) = stage_doc.get("$unionWith") {
                        let (union_collection, union_pipeline): (&str, Option<&Vec<Bson>>) =
                            match union_spec {
                                Bson::String(coll) => (coll.as_str(), None),
                                Bson::Document(d) => (
                                    d.get_str("coll").unwrap_or(""),
                                    d.get_array("pipeline").ok(),
                                ),
                                _ => ("", None),
                            };

                        if !union_collection.is_empty() {
                            // Get documents from the union collection
                            let (_, mut union_docs) = store
                                .find(db, union_collection, &doc! {}, None, None, None)
                                .await;

                            // Apply pipeline stages if any
                            if let Some(pipeline) = union_pipeline {
                                for stage in pipeline {
                                    if let Bson::Document(stage_d) = stage {
                                        if let Ok(match_doc) = stage_d.get_document("$match") {
                                            union_docs
                                                .retain(|d| matches_document_filter(d, match_doc));
                                        } else if let Ok(project_doc) =
                                            stage_d.get_document("$project")
                                        {
                                            union_docs = union_docs
                                                .into_iter()
                                                .map(|d| apply_projection(&d, project_doc))
                                                .collect();
                                        }
                                    }
                                }
                            }

                            // Append union documents
                            docs.extend(union_docs);
                        }
                    }
                    // $replaceWith - replace document with expression result
                    else if let Some(replace_expr) = stage_doc.get("$replaceWith") {
                        docs = docs
                            .into_iter()
                            .filter_map(|d| {
                                match evaluate_expression(replace_expr, &d) {
                                    Bson::Document(new_doc) => Some(new_doc),
                                    _ => None, // Skip non-document results
                                }
                            })
                            .collect();
                    }
                    // $densify - fills gaps in sequence
                    else if let Ok(densify_doc) = stage_doc.get_document("$densify") {
                        let field = densify_doc.get_str("field").unwrap_or("");
                        let partition_by = densify_doc.get("partitionByFields");
                        let range = densify_doc.get_document("range").ok();

                        if !field.is_empty() {
                            if let Some(range_doc) = range {
                                let step = range_doc
                                    .get_i32("step")
                                    .ok()
                                    .or_else(|| range_doc.get_i64("step").ok().map(|v| v as i32))
                                    .unwrap_or(1);
                                let bounds = range_doc.get_str("bounds").unwrap_or("full");

                                // Determine partitions
                                let partitions: Vec<Vec<Document>> =
                                    if let Some(Bson::Array(fields)) = partition_by {
                                        let partition_fields: Vec<&str> = fields
                                            .iter()
                                            .filter_map(|f| {
                                                if let Bson::String(s) = f {
                                                    Some(s.as_str())
                                                } else {
                                                    None
                                                }
                                            })
                                            .collect();

                                        let mut partition_map: std::collections::HashMap<
                                            String,
                                            Vec<Document>,
                                        > = std::collections::HashMap::new();
                                        for d in &docs {
                                            let key: String = partition_fields
                                                .iter()
                                                .map(|f| {
                                                    format!(
                                                        "{:?}",
                                                        d.get(*f).unwrap_or(&Bson::Null)
                                                    )
                                                })
                                                .collect::<Vec<_>>()
                                                .join("_");
                                            partition_map.entry(key).or_default().push(d.clone());
                                        }
                                        partition_map.into_values().collect()
                                    } else {
                                        vec![docs.clone()]
                                    };

                                let mut result_docs = Vec::new();

                                for mut partition in partitions {
                                    // Sort by field
                                    partition.sort_by(|a, b| {
                                        let va = a.get(field).and_then(bson_to_f64);
                                        let vb = b.get(field).and_then(bson_to_f64);
                                        va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
                                    });

                                    if partition.is_empty() {
                                        continue;
                                    }

                                    // Get min/max based on bounds
                                    let (min_val, max_val) = match bounds {
                                        "full" => {
                                            let min = partition
                                                .first()
                                                .and_then(|d| d.get(field))
                                                .and_then(bson_to_f64)
                                                .unwrap_or(0.0);
                                            let max = partition
                                                .last()
                                                .and_then(|d| d.get(field))
                                                .and_then(bson_to_f64)
                                                .unwrap_or(0.0);
                                            (min, max)
                                        }
                                        _ => {
                                            // partition - use existing range
                                            let min = partition
                                                .first()
                                                .and_then(|d| d.get(field))
                                                .and_then(bson_to_f64)
                                                .unwrap_or(0.0);
                                            let max = partition
                                                .last()
                                                .and_then(|d| d.get(field))
                                                .and_then(bson_to_f64)
                                                .unwrap_or(0.0);
                                            (min, max)
                                        }
                                    };

                                    // Build a set of existing values
                                    let existing: std::collections::HashSet<i64> = partition
                                        .iter()
                                        .filter_map(|d| d.get(field).and_then(bson_to_i64))
                                        .collect();

                                    // Generate densified sequence
                                    let mut current = min_val as i64;
                                    let max = max_val as i64;
                                    let mut partition_idx = 0;

                                    while current <= max {
                                        // Find existing document or create new one
                                        while partition_idx < partition.len() {
                                            let doc_val = partition[partition_idx]
                                                .get(field)
                                                .and_then(bson_to_i64);
                                            if let Some(dv) = doc_val {
                                                if dv < current {
                                                    result_docs
                                                        .push(partition[partition_idx].clone());
                                                    partition_idx += 1;
                                                } else {
                                                    break;
                                                }
                                            } else {
                                                partition_idx += 1;
                                            }
                                        }

                                        if existing.contains(&current) {
                                            // Document exists at this value
                                            if partition_idx < partition.len() {
                                                result_docs.push(partition[partition_idx].clone());
                                                partition_idx += 1;
                                            }
                                        } else {
                                            // Create gap-filling document
                                            let mut gap_doc = doc! {};
                                            gap_doc.insert(field, current);
                                            result_docs.push(gap_doc);
                                        }

                                        current += step as i64;
                                    }

                                    // Add remaining documents
                                    while partition_idx < partition.len() {
                                        result_docs.push(partition[partition_idx].clone());
                                        partition_idx += 1;
                                    }
                                }

                                docs = result_docs;
                            }
                        }
                    }
                    // $fill - fill missing field values
                    else if let Ok(fill_doc) = stage_doc.get_document("$fill") {
                        let partition_by = fill_doc.get("partitionBy");
                        let sort_by = fill_doc.get_document("sortBy").ok();
                        let output = fill_doc.get_document("output").ok();

                        // Sort if specified
                        if let Some(sort_doc) = sort_by {
                            docs = apply_sort(docs, sort_doc);
                        }

                        if let Some(output_doc) = output {
                            for (field, fill_spec) in output_doc.iter() {
                                if let Bson::Document(spec) = fill_spec {
                                    let method = spec.get_str("method").ok();
                                    let value = spec.get("value");

                                    match (method, value) {
                                        (Some("locf"), _) => {
                                            // Last observation carried forward
                                            let mut last_value: Option<Bson> = None;
                                            for doc in &mut docs {
                                                if let Some(v) = doc.get(field) {
                                                    if !matches!(v, Bson::Null) {
                                                        last_value = Some(v.clone());
                                                    }
                                                }
                                                if doc.get(field).is_none()
                                                    || matches!(doc.get(field), Some(Bson::Null))
                                                {
                                                    if let Some(lv) = &last_value {
                                                        doc.insert(field, lv.clone());
                                                    }
                                                }
                                            }
                                        }
                                        (Some("linear"), _) => {
                                            // Linear interpolation
                                            let values: Vec<Option<f64>> = docs
                                                .iter()
                                                .map(|d| d.get(field).and_then(bson_to_f64))
                                                .collect();

                                            for i in 0..docs.len() {
                                                if values[i].is_none() {
                                                    // Find prev and next non-null values
                                                    let prev = (0..i)
                                                        .rev()
                                                        .find_map(|j| values[j].map(|v| (j, v)));
                                                    let next = ((i + 1)..docs.len())
                                                        .find_map(|j| values[j].map(|v| (j, v)));

                                                    if let (Some((pi, pv)), Some((ni, nv))) =
                                                        (prev, next)
                                                    {
                                                        let ratio =
                                                            (i - pi) as f64 / (ni - pi) as f64;
                                                        let interpolated = pv + ratio * (nv - pv);
                                                        docs[i].insert(field, interpolated);
                                                    }
                                                }
                                            }
                                        }
                                        (_, Some(v)) => {
                                            // Fill with specific value
                                            for doc in &mut docs {
                                                if doc.get(field).is_none()
                                                    || matches!(doc.get(field), Some(Bson::Null))
                                                {
                                                    doc.insert(field, evaluate_expression(v, doc));
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }

                        let _ = partition_by; // Handle partitioning in a more complex implementation
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
    // DropIndexes
    else if let Ok(collection) = command.get_str("dropIndexes") {
        let index_name = command.get_str("index").ok();

        let result = if let Some(name) = index_name {
            if name == "*" {
                // Drop all indexes except _id
                store.drop_all_indexes(db, collection).await
            } else {
                store.drop_index(db, collection, name).await
            }
        } else {
            // If no index specified, drop all except _id
            store.drop_all_indexes(db, collection).await
        };

        if result {
            doc! {
                "nIndexesWas": 1,
                "ok": 1.0,
            }
        } else {
            doc! {
                "ok": 0.0,
                "errmsg": "index not found",
                "code": 27,
            }
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
    // findAndModify - atomically find and modify a document
    else if let Ok(collection) = command.get_str("findAndModify") {
        let query = command.get_document("query").cloned().unwrap_or_default();
        let sort = command.get_document("sort").ok();
        let update = command.get_document("update").ok();
        let remove = command.get_bool("remove").unwrap_or(false);
        let new_doc = command.get_bool("new").unwrap_or(false);
        let upsert = command.get_bool("upsert").unwrap_or(false);
        let fields = command.get_document("fields").ok();

        let result = store
            .find_and_modify(
                db, collection, &query, sort, update, remove, new_doc, upsert,
            )
            .await;

        if let Some(mut doc) = result {
            // Apply projection if specified
            if let Some(projection) = fields {
                let mut projected = Document::new();
                for (key, value) in projection {
                    match value {
                        Bson::Int32(1) | Bson::Int64(1) | Bson::Boolean(true) => {
                            if let Some(v) = doc.get(key) {
                                projected.insert(key, v.clone());
                            }
                        }
                        _ => {}
                    }
                }
                // Always include _id unless explicitly excluded
                if !projection.contains_key("_id")
                    || !matches!(
                        projection.get("_id"),
                        Some(Bson::Int32(0) | Bson::Int64(0) | Bson::Boolean(false))
                    )
                {
                    if let Some(id) = doc.get("_id") {
                        projected.insert("_id", id.clone());
                    }
                }
                doc = projected;
            }

            doc! {
                "value": doc,
                "ok": 1.0,
            }
        } else {
            doc! {
                "value": Bson::Null,
                "ok": 1.0,
            }
        }
    }
    // distinct - find distinct values for a field
    else if let Ok(collection) = command.get_str("distinct") {
        let key = command.get_str("key").unwrap_or("");
        let query = command.get_document("query").ok();

        let values = store.distinct(db, collection, key, query).await;

        doc! {
            "values": values,
            "ok": 1.0,
        }
    }
    // bulkWrite - execute multiple write operations
    else if command.contains_key("bulkWrite") {
        let ops = command.get_array("ops").ok();
        let ordered = command.get_bool("ordered").unwrap_or(true);
        let ns_info = command.get_array("nsInfo").ok();

        let mut insert_count = 0i64;
        let mut _update_count = 0i64;
        let mut delete_count = 0i64;
        let mut matched_count = 0i64;
        let mut modified_count = 0i64;
        let mut errors: Vec<Document> = Vec::new();

        // Get the collection from nsInfo if available
        let default_collection = ns_info
            .and_then(|arr| arr.first())
            .and_then(|b| b.as_document())
            .and_then(|d| d.get_str("ns").ok())
            .and_then(|ns| ns.split('.').nth(1))
            .unwrap_or("default");

        if let Some(operations) = ops {
            for (idx, op) in operations.iter().enumerate() {
                if let Some(op_doc) = op.as_document() {
                    // Insert operation
                    if let Ok(insert_doc) = op_doc.get_document("insert") {
                        let document = insert_doc
                            .get_document("document")
                            .cloned()
                            .unwrap_or_default();
                        match store.insert_one(db, default_collection, document).await {
                            Ok(_) => insert_count += 1,
                            Err(e) => {
                                if ordered {
                                    errors.push(doc! {
                                        "index": idx as i32,
                                        "code": 11000,
                                        "errmsg": e,
                                    });
                                    break;
                                }
                            }
                        }
                    }
                    // Update operation
                    else if let Ok(update_doc) = op_doc.get_document("update") {
                        let filter = update_doc
                            .get_document("filter")
                            .cloned()
                            .unwrap_or_default();
                        let update_spec = update_doc
                            .get_document("updateMods")
                            .cloned()
                            .unwrap_or_default();
                        let multi = update_doc.get_bool("multi").unwrap_or(false);

                        let (matched, modified) = if multi {
                            store
                                .update_many(db, default_collection, &filter, &update_spec)
                                .await
                        } else {
                            store
                                .update_one(db, default_collection, &filter, &update_spec)
                                .await
                        };
                        matched_count += matched;
                        modified_count += modified;
                        _update_count += 1;
                    }
                    // Delete operation
                    else if let Ok(delete_doc) = op_doc.get_document("delete") {
                        let filter = delete_doc
                            .get_document("filter")
                            .cloned()
                            .unwrap_or_default();
                        let multi = delete_doc.get_bool("multi").unwrap_or(false);

                        let deleted = if multi {
                            store.delete_many(db, default_collection, &filter).await
                        } else {
                            store.delete_one(db, default_collection, &filter).await
                        };
                        delete_count += deleted;
                    }
                }
            }
        }

        doc! {
            "ok": 1.0,
            "nInserted": insert_count,
            "nMatched": matched_count,
            "nModified": modified_count,
            "nDeleted": delete_count,
            "nUpserted": 0,
            "writeErrors": errors.into_iter().map(Bson::Document).collect::<Vec<_>>(),
        }
    }
    // startSession - session management (stub for compatibility)
    else if command.contains_key("startSession") {
        let session_id = ObjectId::new();
        doc! {
            "id": {
                "id": Bson::Binary(bson::Binary {
                    subtype: bson::spec::BinarySubtype::Uuid,
                    bytes: session_id.bytes().to_vec(),
                }),
            },
            "timeoutMinutes": 30,
            "ok": 1.0,
        }
    }
    // endSessions - end one or more sessions
    else if command.contains_key("endSessions") {
        // Sessions are not persisted, so this is a no-op
        doc! { "ok": 1.0 }
    }
    // Session management commands - refreshSessions, killSessions, killAllSessions, killAllSessionsByPattern
    else if command.contains_key("refreshSessions")
        || command.contains_key("killSessions")
        || command.contains_key("killAllSessions")
        || command.contains_key("killAllSessionsByPattern")
    {
        doc! { "ok": 1.0 }
    }
    // currentOp - get current operations
    else if command.contains_key("currentOp") {
        doc! {
            "inprog": Bson::Array(vec![]),
            "ok": 1.0,
        }
    }
    // killOp - kill an operation
    else if command.contains_key("killOp") {
        doc! { "ok": 1.0 }
    }
    // validate - validate a collection
    else if let Ok(collection) = command.get_str("validate") {
        let count = store.count(db, collection, &doc! {}).await;
        doc! {
            "ns": format!("{}.{}", db, collection),
            "nrecords": count,
            "nIndexes": 1,
            "valid": true,
            "ok": 1.0,
        }
    }
    // compact - compact a collection (no-op for in-memory)
    else if command.contains_key("compact") {
        doc! {
            "bytesFreed": 0,
            "ok": 1.0,
        }
    }
    // reIndex - rebuild indexes (stub)
    else if command.contains_key("reIndex") {
        doc! {
            "nIndexesWas": 1,
            "nIndexes": 1,
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
            let should_exclude = project.get(key).is_some_and(|v| {
                matches!(v, Bson::Int32(0) | Bson::Int64(0) | Bson::Boolean(false))
            });
            if !should_exclude {
                result.insert(key, value.clone());
            }
        }
    }

    // Always include _id unless explicitly excluded
    if project
        .get("_id")
        .is_none_or(|v| !matches!(v, Bson::Int32(0) | Bson::Int64(0) | Bson::Boolean(false)))
    {
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

/// Convert BSON to i64 for integer operations
fn bson_to_i64(bson: &Bson) -> Option<i64> {
    match bson {
        Bson::Int32(n) => Some(*n as i64),
        Bson::Int64(n) => Some(*n),
        Bson::Double(n) => Some(*n as i64),
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

/// Apply granularity to bucket boundaries for $bucketAuto
fn apply_granularity(min: f64, max: f64, granularity: &str) -> (f64, f64) {
    match granularity {
        "R5" | "R10" | "R20" | "R40" | "R80" => {
            // Renard series - round to preferred numbers
            let factor = 10_f64.powf(min.log10().floor());
            let adjusted_min = (min / factor).floor() * factor;
            let adjusted_max = (max / factor).ceil() * factor;
            (adjusted_min, adjusted_max)
        }
        "1-2-5" => {
            // 1-2-5 series
            let factor = 10_f64.powf(min.log10().floor());
            let steps = [1.0, 2.0, 5.0, 10.0];
            let normalized_min = min / factor;
            let normalized_max = max / factor;
            let adjusted_min = steps
                .iter()
                .rev()
                .find(|&&s| s <= normalized_min)
                .unwrap_or(&1.0)
                * factor;
            let adjusted_max = steps
                .iter()
                .find(|&&s| s >= normalized_max)
                .unwrap_or(&10.0)
                * factor;
            (adjusted_min, adjusted_max)
        }
        "E6" | "E12" | "E24" | "E48" | "E96" | "E192" => {
            // E series (for resistors/capacitors)
            let factor = 10_f64.powf(min.log10().floor());
            let adjusted_min = (min / factor).floor() * factor;
            let adjusted_max = (max / factor).ceil() * factor;
            (adjusted_min, adjusted_max)
        }
        "POWERSOF2" => {
            // Powers of 2
            let min_power = (min.log2().floor()) as i32;
            let max_power = (max.log2().ceil()) as i32;
            (2_f64.powi(min_power), 2_f64.powi(max_power))
        }
        _ => (min, max), // No adjustment for unknown granularity
    }
}

/// Evaluate an aggregation expression
pub(crate) fn evaluate_expression(expr: &Bson, doc: &Document) -> Bson {
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
                    // Additional arithmetic operators
                    "$abs" => {
                        let val = evaluate_expression(args, doc);
                        match val {
                            Bson::Int32(n) => Bson::Int32(n.abs()),
                            Bson::Int64(n) => Bson::Int64(n.abs()),
                            Bson::Double(n) => Bson::Double(n.abs()),
                            _ => Bson::Null,
                        }
                    }
                    "$ceil" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.ceil())
                        } else {
                            Bson::Null
                        }
                    }
                    "$floor" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.floor())
                        } else {
                            Bson::Null
                        }
                    }
                    "$round" => {
                        if let Bson::Array(arr) = args {
                            if !arr.is_empty() {
                                let val = evaluate_expression(&arr[0], doc);
                                let places = if arr.len() > 1 {
                                    bson_to_f64(&evaluate_expression(&arr[1], doc)).unwrap_or(0.0)
                                        as i32
                                } else {
                                    0
                                };
                                if let Some(n) = bson_to_f64(&val) {
                                    let factor = 10_f64.powi(places);
                                    return Bson::Double((n * factor).round() / factor);
                                }
                            }
                        } else {
                            let val = evaluate_expression(args, doc);
                            if let Some(n) = bson_to_f64(&val) {
                                return Bson::Double(n.round());
                            }
                        }
                        Bson::Null
                    }
                    "$trunc" => {
                        if let Bson::Array(arr) = args {
                            if !arr.is_empty() {
                                let val = evaluate_expression(&arr[0], doc);
                                if let Some(n) = bson_to_f64(&val) {
                                    return Bson::Double(n.trunc());
                                }
                            }
                        } else {
                            let val = evaluate_expression(args, doc);
                            if let Some(n) = bson_to_f64(&val) {
                                return Bson::Double(n.trunc());
                            }
                        }
                        Bson::Null
                    }
                    "$sqrt" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            if n >= 0.0 {
                                return Bson::Double(n.sqrt());
                            }
                        }
                        Bson::Null
                    }
                    "$pow" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let base =
                                    bson_to_f64(&evaluate_expression(&arr[0], doc)).unwrap_or(0.0);
                                let exp =
                                    bson_to_f64(&evaluate_expression(&arr[1], doc)).unwrap_or(0.0);
                                return Bson::Double(base.powf(exp));
                            }
                        }
                        Bson::Null
                    }
                    "$exp" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.exp())
                        } else {
                            Bson::Null
                        }
                    }
                    "$ln" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            if n > 0.0 {
                                return Bson::Double(n.ln());
                            }
                        }
                        Bson::Null
                    }
                    "$log" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let num =
                                    bson_to_f64(&evaluate_expression(&arr[0], doc)).unwrap_or(0.0);
                                let base =
                                    bson_to_f64(&evaluate_expression(&arr[1], doc)).unwrap_or(10.0);
                                if num > 0.0 && base > 0.0 {
                                    return Bson::Double(num.log(base));
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$log10" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            if n > 0.0 {
                                return Bson::Double(n.log10());
                            }
                        }
                        Bson::Null
                    }
                    "$mod" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let a =
                                    bson_to_f64(&evaluate_expression(&arr[0], doc)).unwrap_or(0.0);
                                let b =
                                    bson_to_f64(&evaluate_expression(&arr[1], doc)).unwrap_or(1.0);
                                if b != 0.0 {
                                    return Bson::Double(a % b);
                                }
                            }
                        }
                        Bson::Null
                    }
                    // Trigonometric operators
                    "$sin" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.sin())
                        } else {
                            Bson::Null
                        }
                    }
                    "$cos" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.cos())
                        } else {
                            Bson::Null
                        }
                    }
                    "$tan" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.tan())
                        } else {
                            Bson::Null
                        }
                    }
                    "$asin" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            if (-1.0..=1.0).contains(&n) {
                                return Bson::Double(n.asin());
                            }
                        }
                        Bson::Null
                    }
                    "$acos" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            if (-1.0..=1.0).contains(&n) {
                                return Bson::Double(n.acos());
                            }
                        }
                        Bson::Null
                    }
                    "$atan" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.atan())
                        } else {
                            Bson::Null
                        }
                    }
                    "$atan2" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let y =
                                    bson_to_f64(&evaluate_expression(&arr[0], doc)).unwrap_or(0.0);
                                let x =
                                    bson_to_f64(&evaluate_expression(&arr[1], doc)).unwrap_or(0.0);
                                return Bson::Double(y.atan2(x));
                            }
                        }
                        Bson::Null
                    }
                    "$sinh" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.sinh())
                        } else {
                            Bson::Null
                        }
                    }
                    "$cosh" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.cosh())
                        } else {
                            Bson::Null
                        }
                    }
                    "$tanh" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.tanh())
                        } else {
                            Bson::Null
                        }
                    }
                    "$asinh" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.asinh())
                        } else {
                            Bson::Null
                        }
                    }
                    "$acosh" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            if n >= 1.0 {
                                return Bson::Double(n.acosh());
                            }
                        }
                        Bson::Null
                    }
                    "$atanh" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            if n > -1.0 && n < 1.0 {
                                return Bson::Double(n.atanh());
                            }
                        }
                        Bson::Null
                    }
                    "$degreesToRadians" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.to_radians())
                        } else {
                            Bson::Null
                        }
                    }
                    "$radiansToDegrees" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n.to_degrees())
                        } else {
                            Bson::Null
                        }
                    }
                    // Boolean operators
                    "$and" => {
                        if let Bson::Array(arr) = args {
                            for item in arr {
                                let val = evaluate_expression(item, doc);
                                match val {
                                    Bson::Boolean(false) | Bson::Null => {
                                        return Bson::Boolean(false)
                                    }
                                    _ => {}
                                }
                            }
                            Bson::Boolean(true)
                        } else {
                            Bson::Boolean(false)
                        }
                    }
                    "$or" => {
                        if let Bson::Array(arr) = args {
                            for item in arr {
                                let val = evaluate_expression(item, doc);
                                match val {
                                    Bson::Boolean(false) | Bson::Null => {}
                                    _ => return Bson::Boolean(true),
                                }
                            }
                            Bson::Boolean(false)
                        } else {
                            Bson::Boolean(false)
                        }
                    }
                    "$not" => {
                        if let Bson::Array(arr) = args {
                            if let Some(first) = arr.first() {
                                let val = evaluate_expression(first, doc);
                                match val {
                                    Bson::Boolean(b) => return Bson::Boolean(!b),
                                    Bson::Null => return Bson::Boolean(true),
                                    _ => return Bson::Boolean(false),
                                }
                            }
                        }
                        Bson::Boolean(true)
                    }
                    // Array operators
                    "$first" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::Array(arr) = val {
                            arr.first().cloned().unwrap_or(Bson::Null)
                        } else {
                            Bson::Null
                        }
                    }
                    "$last" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::Array(arr) = val {
                            arr.last().cloned().unwrap_or(Bson::Null)
                        } else {
                            Bson::Null
                        }
                    }
                    "$in" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let needle = evaluate_expression(&arr[0], doc);
                                let haystack = evaluate_expression(&arr[1], doc);
                                if let Bson::Array(h) = haystack {
                                    return Bson::Boolean(h.contains(&needle));
                                }
                            }
                        }
                        Bson::Boolean(false)
                    }
                    "$isArray" => {
                        let val = evaluate_expression(args, doc);
                        Bson::Boolean(matches!(val, Bson::Array(_)))
                    }
                    "$concatArrays" => {
                        if let Bson::Array(arr) = args {
                            let mut result = Vec::new();
                            for item in arr {
                                let val = evaluate_expression(item, doc);
                                if let Bson::Array(a) = val {
                                    result.extend(a);
                                }
                            }
                            Bson::Array(result)
                        } else {
                            Bson::Null
                        }
                    }
                    "$reverseArray" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::Array(mut arr) = val {
                            arr.reverse();
                            Bson::Array(arr)
                        } else {
                            Bson::Null
                        }
                    }
                    "$slice" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() >= 2 {
                                let source = evaluate_expression(&arr[0], doc);
                                let n = bson_to_f64(&evaluate_expression(&arr[1], doc))
                                    .unwrap_or(0.0) as i32;
                                if let Bson::Array(src) = source {
                                    if arr.len() == 2 {
                                        // $slice: [array, n] - first n elements if n > 0, last |n| if n < 0
                                        if n >= 0 {
                                            return Bson::Array(
                                                src.into_iter().take(n as usize).collect(),
                                            );
                                        } else {
                                            let skip = (src.len() as i32 + n).max(0) as usize;
                                            return Bson::Array(
                                                src.into_iter().skip(skip).collect(),
                                            );
                                        }
                                    } else if arr.len() == 3 {
                                        // $slice: [array, position, n]
                                        let pos = n;
                                        let count = bson_to_f64(&evaluate_expression(&arr[2], doc))
                                            .unwrap_or(0.0)
                                            as usize;
                                        let start = if pos >= 0 {
                                            pos as usize
                                        } else {
                                            (src.len() as i32 + pos).max(0) as usize
                                        };
                                        return Bson::Array(
                                            src.into_iter().skip(start).take(count).collect(),
                                        );
                                    }
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$filter" => {
                        if let Bson::Document(filter_doc) = args {
                            let input = filter_doc.get("input");
                            let as_var = filter_doc.get_str("as").unwrap_or("this");
                            let cond = filter_doc.get("cond");

                            if let (Some(input), Some(cond)) = (input, cond) {
                                let input_val = evaluate_expression(input, doc);
                                if let Bson::Array(arr) = input_val {
                                    let filtered: Vec<Bson> = arr
                                        .into_iter()
                                        .filter(|item| {
                                            let mut temp_doc = doc.clone();
                                            temp_doc.insert(as_var.to_string(), item.clone());
                                            matches!(
                                                evaluate_expression(cond, &temp_doc),
                                                Bson::Boolean(true)
                                            )
                                        })
                                        .collect();
                                    return Bson::Array(filtered);
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$map" => {
                        if let Bson::Document(map_doc) = args {
                            let input = map_doc.get("input");
                            let as_var = map_doc.get_str("as").unwrap_or("this");
                            let in_expr = map_doc.get("in");

                            if let (Some(input), Some(in_expr)) = (input, in_expr) {
                                let input_val = evaluate_expression(input, doc);
                                if let Bson::Array(arr) = input_val {
                                    let mapped: Vec<Bson> = arr
                                        .into_iter()
                                        .map(|item| {
                                            let mut temp_doc = doc.clone();
                                            temp_doc.insert(as_var.to_string(), item);
                                            evaluate_expression(in_expr, &temp_doc)
                                        })
                                        .collect();
                                    return Bson::Array(mapped);
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$reduce" => {
                        if let Bson::Document(reduce_doc) = args {
                            let input = reduce_doc.get("input");
                            let initial = reduce_doc.get("initialValue");
                            let in_expr = reduce_doc.get("in");

                            if let (Some(input), Some(initial), Some(in_expr)) =
                                (input, initial, in_expr)
                            {
                                let input_val = evaluate_expression(input, doc);
                                let mut value = evaluate_expression(initial, doc);

                                if let Bson::Array(arr) = input_val {
                                    for item in arr {
                                        let mut temp_doc = doc.clone();
                                        temp_doc.insert("value".to_string(), value.clone());
                                        temp_doc.insert("this".to_string(), item);
                                        value = evaluate_expression(in_expr, &temp_doc);
                                    }
                                }
                                return value;
                            }
                        }
                        Bson::Null
                    }
                    "$indexOfArray" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() >= 2 {
                                let array = evaluate_expression(&arr[0], doc);
                                let search = evaluate_expression(&arr[1], doc);
                                let start = if arr.len() > 2 {
                                    bson_to_f64(&evaluate_expression(&arr[2], doc)).unwrap_or(0.0)
                                        as usize
                                } else {
                                    0
                                };
                                let end = if arr.len() > 3 {
                                    bson_to_f64(&evaluate_expression(&arr[3], doc))
                                        .unwrap_or(i64::MAX as f64)
                                        as usize
                                } else {
                                    usize::MAX
                                };

                                if let Bson::Array(a) = array {
                                    for (i, item) in a.iter().enumerate().skip(start) {
                                        if i >= end {
                                            break;
                                        }
                                        if item == &search {
                                            return Bson::Int32(i as i32);
                                        }
                                    }
                                }
                            }
                        }
                        Bson::Int32(-1)
                    }
                    "$zip" => {
                        if let Bson::Document(zip_doc) = args {
                            let inputs = zip_doc.get_array("inputs").ok();
                            let use_longest = zip_doc.get_bool("useLongestLength").unwrap_or(false);
                            let defaults = zip_doc.get_array("defaults").ok();

                            if let Some(inputs) = inputs {
                                let arrays: Vec<Vec<Bson>> = inputs
                                    .iter()
                                    .map(|input| match evaluate_expression(input, doc) {
                                        Bson::Array(a) => a,
                                        _ => vec![],
                                    })
                                    .collect();

                                if arrays.is_empty() {
                                    return Bson::Array(vec![]);
                                }

                                let max_len = if use_longest {
                                    arrays.iter().map(|a| a.len()).max().unwrap_or(0)
                                } else {
                                    arrays.iter().map(|a| a.len()).min().unwrap_or(0)
                                };

                                let mut result = Vec::new();
                                for i in 0..max_len {
                                    let mut tuple = Vec::new();
                                    for (j, arr) in arrays.iter().enumerate() {
                                        if i < arr.len() {
                                            tuple.push(arr[i].clone());
                                        } else if use_longest {
                                            if let Some(defs) = defaults {
                                                tuple.push(
                                                    defs.get(j).cloned().unwrap_or(Bson::Null),
                                                );
                                            } else {
                                                tuple.push(Bson::Null);
                                            }
                                        }
                                    }
                                    result.push(Bson::Array(tuple));
                                }
                                return Bson::Array(result);
                            }
                        }
                        Bson::Null
                    }
                    "$range" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() >= 2 {
                                let start = bson_to_f64(&evaluate_expression(&arr[0], doc))
                                    .unwrap_or(0.0)
                                    as i32;
                                let end = bson_to_f64(&evaluate_expression(&arr[1], doc))
                                    .unwrap_or(0.0)
                                    as i32;
                                let step = if arr.len() > 2 {
                                    bson_to_f64(&evaluate_expression(&arr[2], doc)).unwrap_or(1.0)
                                        as i32
                                } else {
                                    1
                                };

                                if step == 0 {
                                    return Bson::Null;
                                }

                                let mut result = Vec::new();
                                if step > 0 {
                                    let mut i = start;
                                    while i < end {
                                        result.push(Bson::Int32(i));
                                        i += step;
                                    }
                                } else {
                                    let mut i = start;
                                    while i > end {
                                        result.push(Bson::Int32(i));
                                        i += step;
                                    }
                                }
                                return Bson::Array(result);
                            }
                        }
                        Bson::Null
                    }
                    // String operators
                    "$substr" | "$substrBytes" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 3 {
                                let s = match evaluate_expression(&arr[0], doc) {
                                    Bson::String(s) => s,
                                    _ => return Bson::Null,
                                };
                                let start = bson_to_f64(&evaluate_expression(&arr[1], doc))
                                    .unwrap_or(0.0)
                                    as usize;
                                let len = bson_to_f64(&evaluate_expression(&arr[2], doc))
                                    .unwrap_or(0.0)
                                    as usize;
                                let result: String = s.chars().skip(start).take(len).collect();
                                return Bson::String(result);
                            }
                        }
                        Bson::Null
                    }
                    "$split" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let s = match evaluate_expression(&arr[0], doc) {
                                    Bson::String(s) => s,
                                    _ => return Bson::Null,
                                };
                                let delim = match evaluate_expression(&arr[1], doc) {
                                    Bson::String(s) => s,
                                    _ => return Bson::Null,
                                };
                                let parts: Vec<Bson> = s
                                    .split(&delim)
                                    .map(|p| Bson::String(p.to_string()))
                                    .collect();
                                return Bson::Array(parts);
                            }
                        }
                        Bson::Null
                    }
                    "$strLenBytes" | "$strLenCP" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::String(s) = val {
                            Bson::Int32(s.len() as i32)
                        } else {
                            Bson::Null
                        }
                    }
                    "$trim" => {
                        if let Bson::Document(trim_doc) = args {
                            if let Some(input) = trim_doc.get("input") {
                                let val = evaluate_expression(input, doc);
                                if let Bson::String(s) = val {
                                    return Bson::String(s.trim().to_string());
                                }
                            }
                        } else {
                            let val = evaluate_expression(args, doc);
                            if let Bson::String(s) = val {
                                return Bson::String(s.trim().to_string());
                            }
                        }
                        Bson::Null
                    }
                    "$ltrim" => {
                        if let Bson::Document(trim_doc) = args {
                            if let Some(input) = trim_doc.get("input") {
                                let val = evaluate_expression(input, doc);
                                if let Bson::String(s) = val {
                                    return Bson::String(s.trim_start().to_string());
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$rtrim" => {
                        if let Bson::Document(trim_doc) = args {
                            if let Some(input) = trim_doc.get("input") {
                                let val = evaluate_expression(input, doc);
                                if let Bson::String(s) = val {
                                    return Bson::String(s.trim_end().to_string());
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$regexMatch" => {
                        if let Bson::Document(regex_doc) = args {
                            let input = regex_doc.get("input").and_then(|v| {
                                if let Bson::String(s) = evaluate_expression(v, doc) {
                                    Some(s)
                                } else {
                                    None
                                }
                            });
                            let regex = regex_doc.get_str("regex").ok();
                            if let (Some(input), Some(regex)) = (input, regex) {
                                if let Ok(re) = regex::Regex::new(regex) {
                                    return Bson::Boolean(re.is_match(&input));
                                }
                            }
                        }
                        Bson::Boolean(false)
                    }
                    "$replaceOne" => {
                        if let Bson::Document(replace_doc) = args {
                            let input = replace_doc.get("input").and_then(|v| {
                                if let Bson::String(s) = evaluate_expression(v, doc) {
                                    Some(s)
                                } else {
                                    None
                                }
                            });
                            let find = replace_doc.get_str("find").ok();
                            let replacement = replace_doc.get_str("replacement").ok();
                            if let (Some(input), Some(find), Some(replacement)) =
                                (input, find, replacement)
                            {
                                return Bson::String(input.replacen(find, replacement, 1));
                            }
                        }
                        Bson::Null
                    }
                    "$replaceAll" => {
                        if let Bson::Document(replace_doc) = args {
                            let input = replace_doc.get("input").and_then(|v| {
                                if let Bson::String(s) = evaluate_expression(v, doc) {
                                    Some(s)
                                } else {
                                    None
                                }
                            });
                            let find = replace_doc.get_str("find").ok();
                            let replacement = replace_doc.get_str("replacement").ok();
                            if let (Some(input), Some(find), Some(replacement)) =
                                (input, find, replacement)
                            {
                                return Bson::String(input.replace(find, replacement));
                            }
                        }
                        Bson::Null
                    }
                    "$indexOfBytes" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() >= 2 {
                                let string = match evaluate_expression(&arr[0], doc) {
                                    Bson::String(s) => s,
                                    _ => return Bson::Null,
                                };
                                let substring = match evaluate_expression(&arr[1], doc) {
                                    Bson::String(s) => s,
                                    _ => return Bson::Null,
                                };
                                let start = if arr.len() > 2 {
                                    bson_to_f64(&evaluate_expression(&arr[2], doc)).unwrap_or(0.0)
                                        as usize
                                } else {
                                    0
                                };
                                let end = if arr.len() > 3 {
                                    bson_to_f64(&evaluate_expression(&arr[3], doc))
                                        .unwrap_or(string.len() as f64)
                                        as usize
                                } else {
                                    string.len()
                                };

                                let bytes = string.as_bytes();
                                let search_bytes = substring.as_bytes();

                                if start < bytes.len() && end <= bytes.len() {
                                    if let Some(pos) = bytes[start..end]
                                        .windows(search_bytes.len())
                                        .position(|window| window == search_bytes)
                                    {
                                        return Bson::Int32((start + pos) as i32);
                                    }
                                }
                            }
                        }
                        Bson::Int32(-1)
                    }
                    "$indexOfCP" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() >= 2 {
                                let string = match evaluate_expression(&arr[0], doc) {
                                    Bson::String(s) => s,
                                    _ => return Bson::Null,
                                };
                                let substring = match evaluate_expression(&arr[1], doc) {
                                    Bson::String(s) => s,
                                    _ => return Bson::Null,
                                };
                                let start = if arr.len() > 2 {
                                    bson_to_f64(&evaluate_expression(&arr[2], doc)).unwrap_or(0.0)
                                        as usize
                                } else {
                                    0
                                };
                                let end = if arr.len() > 3 {
                                    let chars: Vec<char> = string.chars().collect();
                                    bson_to_f64(&evaluate_expression(&arr[3], doc))
                                        .unwrap_or(chars.len() as f64)
                                        as usize
                                } else {
                                    string.chars().count()
                                };

                                let chars: Vec<char> = string.chars().collect();
                                let search_chars: Vec<char> = substring.chars().collect();

                                if start < chars.len() && end <= chars.len() {
                                    if let Some(pos) = chars[start..end]
                                        .windows(search_chars.len())
                                        .position(|window| window == search_chars.as_slice())
                                    {
                                        return Bson::Int32((start + pos) as i32);
                                    }
                                }
                            }
                        }
                        Bson::Int32(-1)
                    }
                    "$strcasecmp" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let s1 = match evaluate_expression(&arr[0], doc) {
                                    Bson::String(s) => s.to_lowercase(),
                                    _ => return Bson::Null,
                                };
                                let s2 = match evaluate_expression(&arr[1], doc) {
                                    Bson::String(s) => s.to_lowercase(),
                                    _ => return Bson::Null,
                                };

                                return Bson::Int32(match s1.cmp(&s2) {
                                    std::cmp::Ordering::Less => -1,
                                    std::cmp::Ordering::Equal => 0,
                                    std::cmp::Ordering::Greater => 1,
                                });
                            }
                        }
                        Bson::Null
                    }
                    "$substrCP" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 3 {
                                let s = match evaluate_expression(&arr[0], doc) {
                                    Bson::String(s) => s,
                                    _ => return Bson::Null,
                                };
                                let start = bson_to_f64(&evaluate_expression(&arr[1], doc))
                                    .unwrap_or(0.0)
                                    as usize;
                                let len = bson_to_f64(&evaluate_expression(&arr[2], doc))
                                    .unwrap_or(0.0)
                                    as usize;

                                let chars: Vec<char> = s.chars().collect();
                                let result: String = chars.iter().skip(start).take(len).collect();
                                return Bson::String(result);
                            }
                        }
                        Bson::Null
                    }
                    // Type operators
                    "$type" => {
                        let val = evaluate_expression(args, doc);
                        let type_str = match val {
                            Bson::Double(_) => "double",
                            Bson::String(_) => "string",
                            Bson::Document(_) => "object",
                            Bson::Array(_) => "array",
                            Bson::Binary { .. } => "binData",
                            Bson::ObjectId(_) => "objectId",
                            Bson::Boolean(_) => "bool",
                            Bson::DateTime(_) => "date",
                            Bson::Null => "null",
                            Bson::Int32(_) => "int",
                            Bson::Int64(_) => "long",
                            Bson::Timestamp(_) => "timestamp",
                            _ => "unknown",
                        };
                        Bson::String(type_str.to_string())
                    }
                    "$toDouble" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n)
                        } else {
                            Bson::Null
                        }
                    }
                    "$toLong" => {
                        let val = evaluate_expression(args, doc);
                        match val {
                            Bson::Int32(n) => Bson::Int64(n as i64),
                            Bson::Int64(n) => Bson::Int64(n),
                            Bson::Double(n) => Bson::Int64(n as i64),
                            Bson::String(s) => {
                                s.parse::<i64>().map(Bson::Int64).unwrap_or(Bson::Null)
                            }
                            _ => Bson::Null,
                        }
                    }
                    "$toBool" => {
                        let val = evaluate_expression(args, doc);
                        match val {
                            Bson::Boolean(b) => Bson::Boolean(b),
                            Bson::Int32(n) => Bson::Boolean(n != 0),
                            Bson::Int64(n) => Bson::Boolean(n != 0),
                            Bson::Double(n) => Bson::Boolean(n != 0.0),
                            Bson::String(s) => Bson::Boolean(!s.is_empty()),
                            Bson::Null => Bson::Boolean(false),
                            _ => Bson::Boolean(true),
                        }
                    }
                    "$isNumber" => {
                        let val = evaluate_expression(args, doc);
                        Bson::Boolean(matches!(
                            val,
                            Bson::Int32(_) | Bson::Int64(_) | Bson::Double(_)
                        ))
                    }
                    // Date operators
                    "$year" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::{TimeZone, Utc};
                            let datetime = Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                            Bson::Int32(datetime.format("%Y").to_string().parse().unwrap_or(0))
                        } else {
                            Bson::Null
                        }
                    }
                    "$month" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::{TimeZone, Utc};
                            let datetime = Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                            Bson::Int32(datetime.format("%m").to_string().parse().unwrap_or(0))
                        } else {
                            Bson::Null
                        }
                    }
                    "$dayOfMonth" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::{TimeZone, Utc};
                            let datetime = Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                            Bson::Int32(datetime.format("%d").to_string().parse().unwrap_or(0))
                        } else {
                            Bson::Null
                        }
                    }
                    "$hour" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::{TimeZone, Utc};
                            let datetime = Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                            Bson::Int32(datetime.format("%H").to_string().parse().unwrap_or(0))
                        } else {
                            Bson::Null
                        }
                    }
                    "$minute" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::{TimeZone, Utc};
                            let datetime = Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                            Bson::Int32(datetime.format("%M").to_string().parse().unwrap_or(0))
                        } else {
                            Bson::Null
                        }
                    }
                    "$second" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::{TimeZone, Utc};
                            let datetime = Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                            Bson::Int32(datetime.format("%S").to_string().parse().unwrap_or(0))
                        } else {
                            Bson::Null
                        }
                    }
                    "$dayOfWeek" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::{Datelike, TimeZone, Utc};
                            let datetime = Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                            // MongoDB uses 1 (Sunday) to 7 (Saturday)
                            let dow = datetime.weekday().num_days_from_sunday() + 1;
                            Bson::Int32(dow as i32)
                        } else {
                            Bson::Null
                        }
                    }
                    "$dayOfYear" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::{Datelike, TimeZone, Utc};
                            let datetime = Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                            Bson::Int32(datetime.ordinal() as i32)
                        } else {
                            Bson::Null
                        }
                    }
                    "$dateToString" => {
                        if let Bson::Document(date_doc) = args {
                            let date = date_doc.get("date").map(|d| evaluate_expression(d, doc));
                            let format = date_doc
                                .get_str("format")
                                .unwrap_or("%Y-%m-%dT%H:%M:%S%.3fZ");
                            if let Some(Bson::DateTime(dt)) = date {
                                use chrono::{TimeZone, Utc};
                                let datetime =
                                    Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                                return Bson::String(datetime.format(format).to_string());
                            }
                        }
                        Bson::Null
                    }
                    "$dateFromParts" => {
                        if let Bson::Document(parts_doc) = args {
                            use chrono::{TimeZone, Utc};

                            let year = parts_doc.get_i32("year").unwrap_or(1970);
                            let month = parts_doc.get_i32("month").unwrap_or(1) as u32;
                            let day = parts_doc.get_i32("day").unwrap_or(1) as u32;
                            let hour = parts_doc.get_i32("hour").unwrap_or(0) as u32;
                            let minute = parts_doc.get_i32("minute").unwrap_or(0) as u32;
                            let second = parts_doc.get_i32("second").unwrap_or(0) as u32;
                            let millisecond = parts_doc.get_i32("millisecond").unwrap_or(0) as u32;

                            if let Some(dt) = Utc
                                .with_ymd_and_hms(year, month, day, hour, minute, second)
                                .single()
                            {
                                let dt_with_ms =
                                    dt + chrono::Duration::milliseconds(millisecond as i64);
                                return Bson::DateTime(bson::DateTime::from_millis(
                                    dt_with_ms.timestamp_millis(),
                                ));
                            }
                        }
                        Bson::Null
                    }
                    "$dateFromString" => {
                        if let Bson::Document(date_doc) = args {
                            let date_string = date_doc.get_str("dateString").ok();
                            if let Some(date_str) = date_string {
                                use chrono::{DateTime, Utc};
                                if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
                                    return Bson::DateTime(bson::DateTime::from_millis(
                                        dt.timestamp_millis(),
                                    ));
                                }
                                // Try ISO 8601 without timezone
                                if let Ok(dt) = date_str.parse::<DateTime<Utc>>() {
                                    return Bson::DateTime(bson::DateTime::from_millis(
                                        dt.timestamp_millis(),
                                    ));
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$dateToParts" => {
                        if let Bson::Document(date_doc) = args {
                            if let Some(date) = date_doc.get("date") {
                                let date_val = evaluate_expression(date, doc);
                                if let Bson::DateTime(dt) = date_val {
                                    use chrono::{Datelike, TimeZone, Timelike, Utc};
                                    let datetime =
                                        Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();

                                    return Bson::Document(doc! {
                                        "year": datetime.year(),
                                        "month": datetime.month() as i32,
                                        "day": datetime.day() as i32,
                                        "hour": datetime.hour() as i32,
                                        "minute": datetime.minute() as i32,
                                        "second": datetime.second() as i32,
                                        "millisecond": (datetime.timestamp_subsec_millis()) as i32,
                                    });
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$dateAdd" => {
                        if let Bson::Document(add_doc) = args {
                            if let (Some(start_date), Some(unit), Some(amount)) = (
                                add_doc.get("startDate"),
                                add_doc.get_str("unit").ok(),
                                add_doc.get("amount"),
                            ) {
                                let date_val = evaluate_expression(start_date, doc);
                                let amount_val = bson_to_f64(&evaluate_expression(amount, doc))
                                    .unwrap_or(0.0)
                                    as i64;

                                if let Bson::DateTime(dt) = date_val {
                                    use chrono::{Datelike, Duration, TimeZone, Utc};
                                    let datetime =
                                        Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();

                                    let new_dt = match unit {
                                        "year" => {
                                            datetime.with_year(datetime.year() + amount_val as i32)
                                        }
                                        "month" => {
                                            let total_months = datetime.year() * 12
                                                + datetime.month() as i32
                                                + amount_val as i32;
                                            let new_year = total_months / 12;
                                            let new_month = (total_months % 12) as u32;
                                            datetime
                                                .with_year(new_year)
                                                .and_then(|d| d.with_month(new_month))
                                        }
                                        "week" => Some(datetime + Duration::weeks(amount_val)),
                                        "day" => Some(datetime + Duration::days(amount_val)),
                                        "hour" => Some(datetime + Duration::hours(amount_val)),
                                        "minute" => Some(datetime + Duration::minutes(amount_val)),
                                        "second" => Some(datetime + Duration::seconds(amount_val)),
                                        "millisecond" => {
                                            Some(datetime + Duration::milliseconds(amount_val))
                                        }
                                        _ => None,
                                    };

                                    if let Some(new_datetime) = new_dt {
                                        return Bson::DateTime(bson::DateTime::from_millis(
                                            new_datetime.timestamp_millis(),
                                        ));
                                    }
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$dateDiff" => {
                        if let Bson::Document(diff_doc) = args {
                            if let (Some(start_date), Some(end_date), Some(unit)) = (
                                diff_doc.get("startDate"),
                                diff_doc.get("endDate"),
                                diff_doc.get_str("unit").ok(),
                            ) {
                                let start_val = evaluate_expression(start_date, doc);
                                let end_val = evaluate_expression(end_date, doc);

                                if let (Bson::DateTime(start_dt), Bson::DateTime(end_dt)) =
                                    (start_val, end_val)
                                {
                                    use chrono::{Datelike, TimeZone, Utc};
                                    let start = Utc
                                        .timestamp_millis_opt(start_dt.timestamp_millis())
                                        .unwrap();
                                    let end = Utc
                                        .timestamp_millis_opt(end_dt.timestamp_millis())
                                        .unwrap();
                                    let duration = end.signed_duration_since(start);

                                    let diff = match unit {
                                        "year" => (end.year() - start.year()) as i64,
                                        "month" => {
                                            ((end.year() - start.year()) * 12
                                                + (end.month() as i32 - start.month() as i32))
                                                as i64
                                        }
                                        "week" => duration.num_weeks(),
                                        "day" => duration.num_days(),
                                        "hour" => duration.num_hours(),
                                        "minute" => duration.num_minutes(),
                                        "second" => duration.num_seconds(),
                                        "millisecond" => duration.num_milliseconds(),
                                        _ => 0,
                                    };

                                    return Bson::Int64(diff);
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$dateSubtract" => {
                        if let Bson::Document(sub_doc) = args {
                            if let (Some(start_date), Some(unit), Some(amount)) = (
                                sub_doc.get("startDate"),
                                sub_doc.get_str("unit").ok(),
                                sub_doc.get("amount"),
                            ) {
                                let date_val = evaluate_expression(start_date, doc);
                                let amount_val = -(bson_to_f64(&evaluate_expression(amount, doc))
                                    .unwrap_or(0.0)
                                    as i64);

                                if let Bson::DateTime(dt) = date_val {
                                    use chrono::{Datelike, Duration, TimeZone, Utc};
                                    let datetime =
                                        Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();

                                    let new_dt = match unit {
                                        "year" => {
                                            datetime.with_year(datetime.year() + amount_val as i32)
                                        }
                                        "month" => {
                                            let total_months = datetime.year() * 12
                                                + datetime.month() as i32
                                                + amount_val as i32;
                                            let new_year = total_months / 12;
                                            let new_month = (total_months % 12) as u32;
                                            datetime
                                                .with_year(new_year)
                                                .and_then(|d| d.with_month(new_month))
                                        }
                                        "week" => Some(datetime + Duration::weeks(amount_val)),
                                        "day" => Some(datetime + Duration::days(amount_val)),
                                        "hour" => Some(datetime + Duration::hours(amount_val)),
                                        "minute" => Some(datetime + Duration::minutes(amount_val)),
                                        "second" => Some(datetime + Duration::seconds(amount_val)),
                                        "millisecond" => {
                                            Some(datetime + Duration::milliseconds(amount_val))
                                        }
                                        _ => None,
                                    };

                                    if let Some(new_datetime) = new_dt {
                                        return Bson::DateTime(bson::DateTime::from_millis(
                                            new_datetime.timestamp_millis(),
                                        ));
                                    }
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$dateTrunc" => {
                        if let Bson::Document(trunc_doc) = args {
                            if let (Some(date), Some(unit)) =
                                (trunc_doc.get("date"), trunc_doc.get_str("unit").ok())
                            {
                                let date_val = evaluate_expression(date, doc);

                                if let Bson::DateTime(dt) = date_val {
                                    use chrono::{Datelike, TimeZone, Timelike, Utc};
                                    let datetime =
                                        Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();

                                    let truncated = match unit {
                                        "year" => Utc
                                            .with_ymd_and_hms(datetime.year(), 1, 1, 0, 0, 0)
                                            .single(),
                                        "month" => Utc
                                            .with_ymd_and_hms(
                                                datetime.year(),
                                                datetime.month(),
                                                1,
                                                0,
                                                0,
                                                0,
                                            )
                                            .single(),
                                        "day" => Utc
                                            .with_ymd_and_hms(
                                                datetime.year(),
                                                datetime.month(),
                                                datetime.day(),
                                                0,
                                                0,
                                                0,
                                            )
                                            .single(),
                                        "hour" => Utc
                                            .with_ymd_and_hms(
                                                datetime.year(),
                                                datetime.month(),
                                                datetime.day(),
                                                datetime.hour(),
                                                0,
                                                0,
                                            )
                                            .single(),
                                        "minute" => Utc
                                            .with_ymd_and_hms(
                                                datetime.year(),
                                                datetime.month(),
                                                datetime.day(),
                                                datetime.hour(),
                                                datetime.minute(),
                                                0,
                                            )
                                            .single(),
                                        "second" => Some(datetime.with_nanosecond(0).unwrap()),
                                        _ => None,
                                    };

                                    if let Some(trunc_dt) = truncated {
                                        return Bson::DateTime(bson::DateTime::from_millis(
                                            trunc_dt.timestamp_millis(),
                                        ));
                                    }
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$isoWeek" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::{Datelike, TimeZone, Utc};
                            let datetime = Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                            Bson::Int32(datetime.iso_week().week() as i32)
                        } else {
                            Bson::Null
                        }
                    }
                    "$isoWeekYear" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::{Datelike, TimeZone, Utc};
                            let datetime = Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                            Bson::Int32(datetime.iso_week().year())
                        } else {
                            Bson::Null
                        }
                    }
                    "$isoDayOfWeek" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::{Datelike, TimeZone, Utc};
                            let datetime = Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                            // ISO: Monday = 1, Sunday = 7
                            Bson::Int32(datetime.weekday().number_from_monday() as i32)
                        } else {
                            Bson::Null
                        }
                    }
                    "$millisecond" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::TimeZone;
                            let datetime = chrono::Utc
                                .timestamp_millis_opt(dt.timestamp_millis())
                                .unwrap();
                            Bson::Int32(datetime.timestamp_subsec_millis() as i32)
                        } else {
                            Bson::Null
                        }
                    }
                    "$week" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::DateTime(dt) = val {
                            use chrono::{Datelike, TimeZone, Utc};
                            let datetime = Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap();
                            // Week of year (0-53)
                            let ordinal = datetime.ordinal();
                            let week = (ordinal - 1) / 7;
                            Bson::Int32(week as i32)
                        } else {
                            Bson::Null
                        }
                    }
                    // Object operators
                    "$mergeObjects" => {
                        if let Bson::Array(arr) = args {
                            let mut result = Document::new();
                            for item in arr {
                                let val = evaluate_expression(item, doc);
                                if let Bson::Document(d) = val {
                                    for (k, v) in d {
                                        result.insert(k, v);
                                    }
                                }
                            }
                            Bson::Document(result)
                        } else {
                            Bson::Null
                        }
                    }
                    "$objectToArray" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::Document(d) = val {
                            let arr: Vec<Bson> = d
                                .into_iter()
                                .map(|(k, v)| Bson::Document(doc! { "k": k, "v": v }))
                                .collect();
                            Bson::Array(arr)
                        } else {
                            Bson::Null
                        }
                    }
                    "$arrayToObject" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::Array(arr) = val {
                            let mut result = Document::new();
                            for item in arr {
                                if let Bson::Document(d) = item {
                                    if let (Some(Bson::String(k)), Some(v)) =
                                        (d.get("k"), d.get("v"))
                                    {
                                        result.insert(k.clone(), v.clone());
                                    }
                                } else if let Bson::Array(pair) = item {
                                    if pair.len() == 2 {
                                        if let Bson::String(k) = &pair[0] {
                                            result.insert(k.clone(), pair[1].clone());
                                        }
                                    }
                                }
                            }
                            Bson::Document(result)
                        } else {
                            Bson::Null
                        }
                    }
                    // Set operators
                    "$setUnion" => {
                        if let Bson::Array(arr) = args {
                            let mut result = Vec::new();
                            for item in arr {
                                let val = evaluate_expression(item, doc);
                                if let Bson::Array(a) = val {
                                    for v in a {
                                        if !result.contains(&v) {
                                            result.push(v);
                                        }
                                    }
                                }
                            }
                            Bson::Array(result)
                        } else {
                            Bson::Null
                        }
                    }
                    "$setIntersection" => {
                        if let Bson::Array(arr) = args {
                            if arr.is_empty() {
                                return Bson::Array(vec![]);
                            }
                            let first = evaluate_expression(&arr[0], doc);
                            let mut result = if let Bson::Array(a) = first {
                                a
                            } else {
                                vec![]
                            };

                            for item in arr.iter().skip(1) {
                                let val = evaluate_expression(item, doc);
                                if let Bson::Array(a) = val {
                                    result.retain(|v| a.contains(v));
                                }
                            }
                            Bson::Array(result)
                        } else {
                            Bson::Null
                        }
                    }
                    "$setDifference" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let first = evaluate_expression(&arr[0], doc);
                                let second = evaluate_expression(&arr[1], doc);
                                if let (Bson::Array(a), Bson::Array(b)) = (first, second) {
                                    let result: Vec<Bson> =
                                        a.into_iter().filter(|v| !b.contains(v)).collect();
                                    return Bson::Array(result);
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$setEquals" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() >= 2 {
                                let sets: Vec<Vec<Bson>> = arr
                                    .iter()
                                    .filter_map(|a| {
                                        if let Bson::Array(v) = evaluate_expression(a, doc) {
                                            Some(v)
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();

                                if sets.is_empty() {
                                    return Bson::Boolean(false);
                                }
                                let first = &sets[0];
                                for set in sets.iter().skip(1) {
                                    if set.len() != first.len() {
                                        return Bson::Boolean(false);
                                    }
                                    for v in first {
                                        if !set.contains(v) {
                                            return Bson::Boolean(false);
                                        }
                                    }
                                }
                                return Bson::Boolean(true);
                            }
                        }
                        Bson::Boolean(false)
                    }
                    "$setIsSubset" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let first = evaluate_expression(&arr[0], doc);
                                let second = evaluate_expression(&arr[1], doc);
                                if let (Bson::Array(a), Bson::Array(b)) = (first, second) {
                                    return Bson::Boolean(a.iter().all(|v| b.contains(v)));
                                }
                            }
                        }
                        Bson::Boolean(false)
                    }
                    "$anyElementTrue" => {
                        if let Bson::Array(arr) = args {
                            if let Some(first) = arr.first() {
                                let val = evaluate_expression(first, doc);
                                if let Bson::Array(a) = val {
                                    for v in a {
                                        match v {
                                            Bson::Boolean(true) => return Bson::Boolean(true),
                                            Bson::Int32(n) if n != 0 => return Bson::Boolean(true),
                                            Bson::Int64(n) if n != 0 => return Bson::Boolean(true),
                                            Bson::Double(n) if n != 0.0 => {
                                                return Bson::Boolean(true)
                                            }
                                            Bson::String(s) if !s.is_empty() => {
                                                return Bson::Boolean(true)
                                            }
                                            Bson::Document(_) | Bson::Array(_) => {
                                                return Bson::Boolean(true)
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                        Bson::Boolean(false)
                    }
                    "$allElementsTrue" => {
                        if let Bson::Array(arr) = args {
                            if let Some(first) = arr.first() {
                                let val = evaluate_expression(first, doc);
                                if let Bson::Array(a) = val {
                                    for v in a {
                                        match v {
                                            Bson::Boolean(false) | Bson::Null => {
                                                return Bson::Boolean(false)
                                            }
                                            Bson::Int32(0) | Bson::Int64(0) => {
                                                return Bson::Boolean(false)
                                            }
                                            Bson::Double(0.0) => return Bson::Boolean(false),
                                            _ => {}
                                        }
                                    }
                                    return Bson::Boolean(true);
                                }
                            }
                        }
                        Bson::Boolean(false)
                    }
                    // Comparison
                    "$cmp" => {
                        if let Bson::Array(arr) = args {
                            if arr.len() == 2 {
                                let a = evaluate_expression(&arr[0], doc);
                                let b = evaluate_expression(&arr[1], doc);
                                let cmp = compare_bson_values(Some(&a), Some(&b));
                                return match cmp {
                                    std::cmp::Ordering::Less => Bson::Int32(-1),
                                    std::cmp::Ordering::Equal => Bson::Int32(0),
                                    std::cmp::Ordering::Greater => Bson::Int32(1),
                                };
                            }
                        }
                        Bson::Int32(0)
                    }
                    // Switch/case
                    "$switch" => {
                        if let Bson::Document(switch_doc) = args {
                            if let Ok(branches) = switch_doc.get_array("branches") {
                                for branch in branches {
                                    if let Bson::Document(b) = branch {
                                        if let Some(case_expr) = b.get("case") {
                                            let case_result = evaluate_expression(case_expr, doc);
                                            if matches!(case_result, Bson::Boolean(true)) {
                                                if let Some(then_expr) = b.get("then") {
                                                    return evaluate_expression(then_expr, doc);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if let Some(default) = switch_doc.get("default") {
                                return evaluate_expression(default, doc);
                            }
                        }
                        Bson::Null
                    }
                    "$let" => {
                        if let Bson::Document(let_doc) = args {
                            let vars = let_doc.get_document("vars").ok();
                            let in_expr = let_doc.get("in");

                            if let (Some(vars), Some(in_expr)) = (vars, in_expr) {
                                let mut temp_doc = doc.clone();
                                for (var_name, var_expr) in vars {
                                    let value = evaluate_expression(var_expr, doc);
                                    temp_doc.insert(var_name.clone(), value);
                                }
                                return evaluate_expression(in_expr, &temp_doc);
                            }
                        }
                        Bson::Null
                    }

                    // Object expressions: $setField, $getField, $unsetField
                    "$setField" => {
                        if let Bson::Document(set_doc) = args {
                            let field = set_doc.get("field").and_then(|f| match f {
                                Bson::String(s) => Some(s.as_str()),
                                _ => None,
                            });
                            let input = set_doc.get("input");
                            let value = set_doc.get("value");

                            if let (Some(field_name), Some(input_val), Some(val)) =
                                (field, input, value)
                            {
                                let mut result = match evaluate_expression(input_val, doc) {
                                    Bson::Document(d) => d,
                                    _ => return Bson::Null,
                                };
                                result
                                    .insert(field_name.to_string(), evaluate_expression(val, doc));
                                return Bson::Document(result);
                            }
                        }
                        Bson::Null
                    }
                    "$getField" => {
                        if let Bson::Document(get_doc) = args {
                            let field = get_doc.get("field").and_then(|f| match f {
                                Bson::String(s) => Some(s.as_str()),
                                _ => None,
                            });
                            let input = get_doc.get("input");

                            if let (Some(field_name), Some(input_val)) = (field, input) {
                                let result = evaluate_expression(input_val, doc);
                                if let Bson::Document(d) = result {
                                    return d.get(field_name).cloned().unwrap_or(Bson::Null);
                                }
                            }
                        } else if let Bson::String(field_name) = args {
                            // Simple form: { $getField: "fieldName" }
                            return doc.get(field_name).cloned().unwrap_or(Bson::Null);
                        }
                        Bson::Null
                    }
                    "$unsetField" => {
                        if let Bson::Document(unset_doc) = args {
                            let field = unset_doc.get("field").and_then(|f| match f {
                                Bson::String(s) => Some(s.as_str()),
                                _ => None,
                            });
                            let input = unset_doc.get("input");

                            if let (Some(field_name), Some(input_val)) = (field, input) {
                                let mut result = match evaluate_expression(input_val, doc) {
                                    Bson::Document(d) => d,
                                    _ => return Bson::Null,
                                };
                                result.remove(field_name);
                                return Bson::Document(result);
                            }
                        }
                        Bson::Null
                    }

                    // Regex expressions: $regexFind, $regexFindAll
                    "$regexFind" => {
                        if let Bson::Document(regex_doc) = args {
                            let input = regex_doc.get("input").and_then(|i| {
                                match evaluate_expression(i, doc) {
                                    Bson::String(s) => Some(s),
                                    _ => None,
                                }
                            });
                            let regex_str = regex_doc.get("regex").and_then(|r| match r {
                                Bson::String(s) => Some(s.clone()),
                                Bson::RegularExpression(re) => Some(re.pattern.clone()),
                                _ => None,
                            });
                            let options = regex_doc.get("options").and_then(|o| match o {
                                Bson::String(s) => Some(s.clone()),
                                _ => None,
                            });

                            if let (Some(input_str), Some(pattern)) = (input, regex_str) {
                                let mut regex_pattern = pattern;
                                if let Some(opts) = options {
                                    regex_pattern = format!("(?{}){}", opts, regex_pattern);
                                }
                                if let Ok(re) = regex::Regex::new(&regex_pattern) {
                                    if let Some(mat) = re.find(&input_str) {
                                        let captures: Vec<Bson> = re
                                            .captures(&input_str)
                                            .map(|caps| {
                                                caps.iter()
                                                    .skip(1)
                                                    .map(|m| {
                                                        m.map(|m| {
                                                            Bson::String(m.as_str().to_string())
                                                        })
                                                        .unwrap_or(Bson::Null)
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default();
                                        return Bson::Document(doc! {
                                            "match": mat.as_str(),
                                            "idx": mat.start() as i32,
                                            "captures": captures
                                        });
                                    }
                                }
                            }
                        }
                        Bson::Null
                    }
                    "$regexFindAll" => {
                        if let Bson::Document(regex_doc) = args {
                            let input = regex_doc.get("input").and_then(|i| {
                                match evaluate_expression(i, doc) {
                                    Bson::String(s) => Some(s),
                                    _ => None,
                                }
                            });
                            let regex_str = regex_doc.get("regex").and_then(|r| match r {
                                Bson::String(s) => Some(s.clone()),
                                Bson::RegularExpression(re) => Some(re.pattern.clone()),
                                _ => None,
                            });
                            let options = regex_doc.get("options").and_then(|o| match o {
                                Bson::String(s) => Some(s.clone()),
                                _ => None,
                            });

                            if let (Some(input_str), Some(pattern)) = (input, regex_str) {
                                let mut regex_pattern = pattern;
                                if let Some(opts) = options {
                                    regex_pattern = format!("(?{}){}", opts, regex_pattern);
                                }
                                if let Ok(re) = regex::Regex::new(&regex_pattern) {
                                    let matches: Vec<Bson> = re
                                        .find_iter(&input_str)
                                        .map(|mat| {
                                            let captures: Vec<Bson> = re
                                                .captures(mat.as_str())
                                                .map(|caps| {
                                                    caps.iter()
                                                        .skip(1)
                                                        .map(|m| {
                                                            m.map(|m| {
                                                                Bson::String(m.as_str().to_string())
                                                            })
                                                            .unwrap_or(Bson::Null)
                                                        })
                                                        .collect()
                                                })
                                                .unwrap_or_default();
                                            Bson::Document(doc! {
                                                "match": mat.as_str(),
                                                "idx": mat.start() as i32,
                                                "captures": captures
                                            })
                                        })
                                        .collect();
                                    return Bson::Array(matches);
                                }
                            }
                        }
                        Bson::Array(vec![])
                    }

                    // Type expressions: $convert, $isBool, $isDate, $toDecimal, $toObjectId
                    "$convert" => {
                        if let Bson::Document(conv_doc) = args {
                            let input = conv_doc.get("input").map(|i| evaluate_expression(i, doc));
                            let to = conv_doc.get("to").and_then(|t| match t {
                                Bson::String(s) => Some(s.as_str()),
                                Bson::Int32(i) => match i {
                                    1 => Some("double"),
                                    2 => Some("string"),
                                    7 => Some("objectId"),
                                    8 => Some("bool"),
                                    9 => Some("date"),
                                    16 => Some("int"),
                                    18 => Some("long"),
                                    19 => Some("decimal"),
                                    _ => None,
                                },
                                _ => None,
                            });
                            let on_error = conv_doc.get("onError");
                            let on_null = conv_doc.get("onNull");

                            if let (Some(input_val), Some(target_type)) = (input, to) {
                                if matches!(input_val, Bson::Null) {
                                    return on_null
                                        .map(|v| evaluate_expression(v, doc))
                                        .unwrap_or(Bson::Null);
                                }
                                match target_type {
                                    "string" => {
                                        return Bson::String(format!("{:?}", input_val));
                                    }
                                    "double" => {
                                        if let Some(n) = bson_to_f64(&input_val) {
                                            return Bson::Double(n);
                                        }
                                    }
                                    "int" => {
                                        if let Some(n) = bson_to_i64(&input_val) {
                                            return Bson::Int32(n as i32);
                                        }
                                    }
                                    "long" => {
                                        if let Some(n) = bson_to_i64(&input_val) {
                                            return Bson::Int64(n);
                                        }
                                    }
                                    "bool" => {
                                        return Bson::Boolean(match &input_val {
                                            Bson::Boolean(b) => *b,
                                            Bson::Int32(i) => *i != 0,
                                            Bson::Int64(i) => *i != 0,
                                            Bson::Double(d) => *d != 0.0,
                                            Bson::String(s) => !s.is_empty(),
                                            Bson::Null => false,
                                            _ => true,
                                        });
                                    }
                                    "objectId" => {
                                        if let Bson::String(s) = &input_val {
                                            if let Ok(oid) = bson::oid::ObjectId::parse_str(s) {
                                                return Bson::ObjectId(oid);
                                            }
                                        }
                                    }
                                    "date" => {
                                        if let Bson::String(s) = &input_val {
                                            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s)
                                            {
                                                return Bson::DateTime(
                                                    bson::DateTime::from_millis(
                                                        dt.with_timezone(&chrono::Utc)
                                                            .timestamp_millis(),
                                                    ),
                                                );
                                            }
                                        } else if let Some(n) = bson_to_i64(&input_val) {
                                            return Bson::DateTime(bson::DateTime::from_millis(n));
                                        }
                                    }
                                    "decimal" => {
                                        if let Some(n) = bson_to_f64(&input_val) {
                                            return Bson::Double(n); // Approximate decimal
                                        }
                                    }
                                    _ => {}
                                }
                                return on_error
                                    .map(|v| evaluate_expression(v, doc))
                                    .unwrap_or(Bson::Null);
                            }
                        }
                        Bson::Null
                    }
                    "$isBool" => {
                        let val = evaluate_expression(args, doc);
                        Bson::Boolean(matches!(val, Bson::Boolean(_)))
                    }
                    "$isDate" => {
                        let val = evaluate_expression(args, doc);
                        Bson::Boolean(matches!(val, Bson::DateTime(_)))
                    }
                    "$toDecimal" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_f64(&val) {
                            Bson::Double(n)
                        } else {
                            Bson::Null
                        }
                    }
                    "$toObjectId" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::String(s) = val {
                            if let Ok(oid) = bson::oid::ObjectId::parse_str(&s) {
                                return Bson::ObjectId(oid);
                            }
                        }
                        Bson::Null
                    }
                    "$toDate" | "$toISODate" => {
                        let val = evaluate_expression(args, doc);
                        match val {
                            Bson::String(s) => {
                                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&s) {
                                    Bson::DateTime(bson::DateTime::from_millis(
                                        dt.with_timezone(&chrono::Utc).timestamp_millis(),
                                    ))
                                } else {
                                    Bson::Null
                                }
                            }
                            Bson::Int64(n) => Bson::DateTime(bson::DateTime::from_millis(n)),
                            Bson::Int32(n) => Bson::DateTime(bson::DateTime::from_millis(n as i64)),
                            Bson::Double(n) => {
                                Bson::DateTime(bson::DateTime::from_millis(n as i64))
                            }
                            _ => Bson::Null,
                        }
                    }
                    "$toUUID" => {
                        let val = evaluate_expression(args, doc);
                        if let Bson::String(s) = val {
                            // Parse UUID string format
                            if s.len() == 36 && s.chars().filter(|c| *c == '-').count() == 4 {
                                return Bson::String(s); // Return as string UUID
                            }
                        }
                        Bson::Null
                    }

                    // Miscellaneous expressions: $rand, $meta, bitwise ops, $sortArray
                    "$rand" => {
                        use rand::Rng;
                        let mut rng = rand::thread_rng();
                        Bson::Double(rng.gen::<f64>())
                    }
                    "$sampleRate" => {
                        use rand::Rng;
                        if let Some(rate) = bson_to_f64(args) {
                            let mut rng = rand::thread_rng();
                            Bson::Boolean(rng.gen::<f64>() < rate)
                        } else {
                            Bson::Boolean(false)
                        }
                    }
                    "$meta" => {
                        // $meta provides access to document metadata
                        if let Bson::String(meta_type) = args {
                            match meta_type.as_str() {
                                "textScore" => {
                                    // Return text search score if available
                                    doc.get("$textScore").cloned().unwrap_or(Bson::Double(0.0))
                                }
                                "indexKey" => {
                                    // Return index key used
                                    doc.get("$indexKey").cloned().unwrap_or(Bson::Null)
                                }
                                "searchScore" => doc
                                    .get("$searchScore")
                                    .cloned()
                                    .unwrap_or(Bson::Double(0.0)),
                                "searchHighlights" => doc
                                    .get("$searchHighlights")
                                    .cloned()
                                    .unwrap_or(Bson::Array(vec![])),
                                _ => Bson::Null,
                            }
                        } else {
                            Bson::Null
                        }
                    }
                    "$bitAnd" => {
                        if let Bson::Array(arr) = args {
                            let values: Vec<i64> = arr
                                .iter()
                                .filter_map(|v| bson_to_i64(&evaluate_expression(v, doc)))
                                .collect();
                            if values.is_empty() {
                                return Bson::Null;
                            }
                            let result = values.iter().fold(!0i64, |acc, v| acc & v);
                            Bson::Int64(result)
                        } else {
                            Bson::Null
                        }
                    }
                    "$bitOr" => {
                        if let Bson::Array(arr) = args {
                            let values: Vec<i64> = arr
                                .iter()
                                .filter_map(|v| bson_to_i64(&evaluate_expression(v, doc)))
                                .collect();
                            if values.is_empty() {
                                return Bson::Null;
                            }
                            let result = values.iter().fold(0i64, |acc, v| acc | v);
                            Bson::Int64(result)
                        } else {
                            Bson::Null
                        }
                    }
                    "$bitXor" => {
                        if let Bson::Array(arr) = args {
                            let values: Vec<i64> = arr
                                .iter()
                                .filter_map(|v| bson_to_i64(&evaluate_expression(v, doc)))
                                .collect();
                            if values.is_empty() {
                                return Bson::Null;
                            }
                            let result = values.iter().fold(0i64, |acc, v| acc ^ v);
                            Bson::Int64(result)
                        } else {
                            Bson::Null
                        }
                    }
                    "$bitNot" => {
                        let val = evaluate_expression(args, doc);
                        if let Some(n) = bson_to_i64(&val) {
                            Bson::Int64(!n)
                        } else {
                            Bson::Null
                        }
                    }
                    "$sortArray" => {
                        if let Bson::Document(sort_doc) = args {
                            let input = sort_doc.get("input");
                            let sort_by = sort_doc.get("sortBy");

                            if let (Some(input_val), Some(sort_spec)) = (input, sort_by) {
                                let mut arr = match evaluate_expression(input_val, doc) {
                                    Bson::Array(a) => a,
                                    _ => return Bson::Null,
                                };

                                // Parse sort specification
                                let sort_fields: Vec<(String, i32)> = match sort_spec {
                                    Bson::Document(d) => d
                                        .iter()
                                        .filter_map(|(k, v)| {
                                            bson_to_i64(v).map(|n| (k.clone(), n as i32))
                                        })
                                        .collect(),
                                    Bson::Int32(n) => vec![("".to_string(), *n)],
                                    Bson::Int64(n) => vec![("".to_string(), *n as i32)],
                                    _ => return Bson::Array(arr),
                                };

                                arr.sort_by(|a, b| {
                                    for (field, direction) in &sort_fields {
                                        let val_a = if field.is_empty() {
                                            Some(a.clone())
                                        } else if let Bson::Document(d) = a {
                                            Some(d.get(field).cloned().unwrap_or(Bson::Null))
                                        } else {
                                            Some(Bson::Null)
                                        };
                                        let val_b = if field.is_empty() {
                                            Some(b.clone())
                                        } else if let Bson::Document(d) = b {
                                            Some(d.get(field).cloned().unwrap_or(Bson::Null))
                                        } else {
                                            Some(Bson::Null)
                                        };

                                        let cmp =
                                            compare_bson_values(val_a.as_ref(), val_b.as_ref());
                                        if cmp != std::cmp::Ordering::Equal {
                                            return if *direction >= 0 {
                                                cmp
                                            } else {
                                                cmp.reverse()
                                            };
                                        }
                                    }
                                    std::cmp::Ordering::Equal
                                });

                                return Bson::Array(arr);
                            }
                        }
                        Bson::Null
                    }

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
