use super::protocol::{
    MongoCodec, MongoHeader, MongoMessage, MsgSection, OP_MSG, OP_REPLY,
};
use bson::doc;
use futures::{SinkExt, StreamExt};
use orbit_shared::OrbitResult;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;
use tracing::{debug, error, info};

pub struct MongoDbServer {
    address: String,
}

impl MongoDbServer {
    pub fn new(address: String) -> Self {
        Self { address }
    }

    pub async fn run(&self) -> OrbitResult<()> {
        let listener = TcpListener::bind(&self.address).await?;
        info!("MongoDB server listening on {}", self.address);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    debug!("New MongoDB connection from {}", addr);
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(socket).await {
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

async fn handle_connection(socket: TcpStream) -> OrbitResult<()> {
    info!("MongoDB: Starting connection handler");

    let mut framed = Framed::new(socket, MongoCodec::new());

    while let Some(result) = framed.next().await {
        match result {
            Ok(message) => {
                match message {
                    MongoMessage::Query {
                        header,
                        full_collection_name,
                        query,
                        ..
                    } => {
                        debug!("Received OP_QUERY: {} {:?}", full_collection_name, query);
                        
                        // Handle handshake (isMaster / hello / ismaster)
                        // Older clients use OP_QUERY for isMaster
                        if full_collection_name.ends_with(".$cmd") {
                            if query.contains_key("isMaster") || query.contains_key("ismaster") || query.contains_key("hello") {
                                let response_doc = doc! {
                                    "ismaster": true,
                                    "maxBsonObjectSize": 16777216,
                                    "maxMessageSizeBytes": 48000000,
                                    "maxWriteBatchSize": 100000,
                                    "localTime": bson::DateTime::now(),
                                    "logicalSessionTimeoutMinutes": 30,
                                    "minWireVersion": 0,
                                    "maxWireVersion": 13,
                                    "readOnly": false,
                                    "ok": 1.0,
                                };
                                
                                let reply = MongoMessage::Reply {
                                    header: MongoHeader {
                                        message_length: 0, // Calculated by encoder
                                        request_id: header.request_id + 1, // Just a guess
                                        response_to: header.request_id,
                                        op_code: OP_REPLY,
                                    },
                                    response_flags: 0,
                                    cursor_id: 0,
                                    starting_from: 0,
                                    number_returned: 1,
                                    documents: vec![response_doc],
                                };
                                
                                framed.send(reply).await?;
                            } else {
                                // Generic command response (mock)
                                let response_doc = doc! { "ok": 1.0 };
                                let reply = MongoMessage::Reply {
                                    header: MongoHeader {
                                        message_length: 0,
                                        request_id: 0,
                                        response_to: header.request_id,
                                        op_code: OP_REPLY,
                                    },
                                    response_flags: 0,
                                    cursor_id: 0,
                                    starting_from: 0,
                                    number_returned: 1,
                                    documents: vec![response_doc],
                                };
                                framed.send(reply).await?;
                            }
                        } else {
                            // Regular query
                            let _response_doc = doc! { "ok": 1.0 };
                            let reply = MongoMessage::Reply {
                                header: MongoHeader {
                                    message_length: 0,
                                    request_id: 0,
                                    response_to: header.request_id,
                                    op_code: OP_REPLY,
                                },
                                response_flags: 0,
                                cursor_id: 0,
                                starting_from: 0,
                                number_returned: 0, // Empty result for now
                                documents: vec![],
                            };
                            framed.send(reply).await?;
                        }
                    }
                    MongoMessage::Msg { header, sections, .. } => {
                        debug!("Received OP_MSG");
                        
                        // Find the body section
                        let mut command_doc = doc! {};
                        for section in &sections {
                            if let MsgSection::Body(doc) = section {
                                command_doc = doc.clone();
                                break;
                            }
                        }
                        
                        debug!("Command: {:?}", command_doc);

                        let mut response_doc = doc! { "ok": 1.0 };

                        if command_doc.contains_key("isMaster") || command_doc.contains_key("ismaster") || command_doc.contains_key("hello") {
                             response_doc = doc! {
                                "ismaster": true,
                                "maxBsonObjectSize": 16777216,
                                "maxMessageSizeBytes": 48000000,
                                "maxWriteBatchSize": 100000,
                                "localTime": bson::DateTime::now(),
                                "logicalSessionTimeoutMinutes": 30,
                                "minWireVersion": 0,
                                "maxWireVersion": 13,
                                "readOnly": false,
                                "ok": 1.0,
                            };
                        } else if command_doc.contains_key("insert") {
                            // Mock insert response
                            let n = if let Some(docs) = command_doc.get_array("documents").ok() {
                                docs.len() as i32
                            } else {
                                // Check for document sequence
                                let mut count = 0;
                                for section in &sections {
                                    if let MsgSection::DocumentSequence { documents, .. } = section {
                                        count += documents.len();
                                    }
                                }
                                count as i32
                            };
                            
                            response_doc = doc! {
                                "n": n,
                                "ok": 1.0,
                            };
                        } else if command_doc.contains_key("find") {
                            // Mock find response (empty cursor)
                            response_doc = doc! {
                                "cursor": {
                                    "id": 0i64,
                                    "ns": format!("{}.{}", command_doc.get_str("$db").unwrap_or("test"), command_doc.get_str("find").unwrap_or("collection")),
                                    "firstBatch": []
                                },
                                "ok": 1.0,
                            };
                        } else if command_doc.contains_key("buildInfo") {
                             response_doc = doc! {
                                "version": "5.0.0",
                                "gitVersion": "orbit-mock",
                                "modules": [],
                                "allocator": "system",
                                "javascriptEngine": "mozjs",
                                "sysInfo": "deprecated",
                                "versionArray": [5, 0, 0, 0],
                                "openssl": {
                                    "running": "OpenSSL 1.1.1",
                                    "compiled": "OpenSSL 1.1.1"
                                },
                                "buildEnvironment": {
                                    "distmod": "ubuntu2004",
                                    "distarch": "x86_64",
                                    "cc": "/opt/mongodbtoolchain/v3/bin/gcc: gcc (GCC) 8.5.0",
                                    "ccflags": "-Werror -include mongo/platform/basic.h",
                                    "cxx": "/opt/mongodbtoolchain/v3/bin/g++: g++ (GCC) 8.5.0",
                                    "cxxflags": "-Werror -include mongo/platform/basic.h",
                                    "linkflags": "-Wl,--as-needed",
                                    "target_arch": "x86_64",
                                    "target_os": "linux",
                                    "cppdefines": "SAFEINT_USE_INTRINSICS 0 PCRE_STATIC 1"
                                },
                                "bits": 64,
                                "debug": false,
                                "maxBsonObjectSize": 16777216,
                                "storageEngines": [
                                    "devnull",
                                    "ephemeralForTest",
                                    "wiredTiger"
                                ],
                                "ok": 1.0
                            };
                        }

                        let reply = MongoMessage::Msg {
                            header: MongoHeader {
                                message_length: 0,
                                request_id: header.request_id + 1, // Just a guess
                                response_to: header.request_id,
                                op_code: OP_MSG,
                            },
                            flag_bits: 0,
                            sections: vec![MsgSection::Body(response_doc)],
                            checksum: None,
                        };
                        
                        framed.send(reply).await?;
                    }
                    _ => {
                        debug!("Received unknown or unsupported message");
                    }
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
