use orbit_client::ActorRegistry;
use orbit_proto::{
    connection_service_server, ConnectionInfoRequestProto, ConnectionInfoResponseProto,
    MessageContentProto, MessageProto, NodeIdProto,
};
use orbit_shared::OrbitError;
use std::sync::Arc;
use tokio::sync::mpsc;
use tonic::{Request, Response, Status, Streaming};

/// Server-side connection service implementation
#[derive(Clone)]
pub struct ServerConnectionService {
    registry: Arc<ActorRegistry>,
    local_node_id: NodeIdProto,
}

impl std::fmt::Debug for ServerConnectionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConnectionService")
            .field("local_node_id", &self.local_node_id)
            .finish()
    }
}

impl ServerConnectionService {
    pub fn new(registry: Arc<ActorRegistry>, local_node_id: NodeIdProto) -> Self {
        Self {
            registry,
            local_node_id,
        }
    }

    pub fn handle_local_stream(
        &self,
        mut stream: mpsc::Receiver<MessageProto>,
        tx: mpsc::Sender<Result<MessageProto, Status>>,
    ) {
        let registry = self.registry.clone();
        let local_node_id = self.local_node_id.clone();

        tokio::spawn(async move {
            while let Some(message) = stream.recv().await {
                // Bridge: Unbounded -> Bounded
                let (bridge_tx, mut bridge_rx) = mpsc::unbounded_channel();
                let tx_clone = tx.clone();

                tokio::spawn(async move {
                    while let Some(msg) = bridge_rx.recv().await {
                        if let Err(_) = tx_clone.send(msg).await {
                            break;
                        }
                    }
                });

                Self::process_message(registry.clone(), local_node_id.clone(), message, bridge_tx)
                    .await;
            }
        });
    }

    async fn process_message(
        registry: Arc<ActorRegistry>,
        local_node_id: NodeIdProto,
        message: MessageProto,
        tx: mpsc::UnboundedSender<Result<MessageProto, Status>>,
    ) {
        let message_id = message.message_id;
        let source = message.source.clone();
        let _target = message.target.clone();

        if let Some(content) = message.content {
            if let Some(inner) = content.content {
                match inner {
                    orbit_proto::message_content_proto::Content::InvocationRequest(req) => {
                        tokio::spawn(async move {
                            let result = async {
                                // Convert reference
                                let reference_proto = req.reference.ok_or_else(|| OrbitError::internal("Missing reference"))?;
                                let reference = orbit_proto::converters::AddressableReferenceConverter::from_proto(&reference_proto)
                                    .map_err(|e| OrbitError::internal(format!("Invalid reference: {}", e)))?;

                                // Deserialize arguments
                                let args: Vec<serde_json::Value> = serde_json::from_str(&req.arguments)
                                    .map_err(|e| OrbitError::SerializationError(e))?;

                                // Get actor instance
                                let instance = registry.get_or_create_instance(reference).await?;

                                // Invoke method
                                instance.implementation.handle_invocation(&req.method, args).await
                            }.await;

                            // Create response
                            let response_content = match result {
                                Ok(value) => {
                                    let value_str = serde_json::to_string(&value).unwrap_or_default();
                                    orbit_proto::message_content_proto::Content::InvocationResponse(
                                        orbit_proto::InvocationResponseProto {
                                            value: value_str,
                                        }
                                    )
                                }
                                Err(e) => {
                                    orbit_proto::message_content_proto::Content::InvocationResponseError(
                                        orbit_proto::InvocationResponseErrorProto {
                                            description: e.to_string(),
                                            platform: "orbit-server".to_string(),
                                        }
                                    )
                                }
                            };

                            let response = MessageProto {
                                message_id,
                                source: Some(local_node_id),
                                target: Some(orbit_proto::MessageTargetProto {
                                    target: Some(
                                        orbit_proto::message_target_proto::Target::UnicastTarget(
                                            orbit_proto::message_target_proto::Unicast {
                                                target: source,
                                            },
                                        ),
                                    ),
                                }),
                                content: Some(MessageContentProto {
                                    content: Some(response_content),
                                }),
                                attempts: 0,
                            };

                            let _ = tx.send(Ok(response));
                        });
                    }
                    _ => {
                        // Echo other messages or ignore
                        tracing::debug!("Received non-invocation message: {:?}", inner);
                    }
                }
            }
        }
    }
}

#[tonic::async_trait]
impl connection_service_server::ConnectionService for ServerConnectionService {
    type OpenStreamStream =
        tokio_stream::wrappers::UnboundedReceiverStream<Result<MessageProto, Status>>;

    async fn open_stream(
        &self,
        request: Request<Streaming<MessageProto>>,
    ) -> Result<Response<Self::OpenStreamStream>, Status> {
        let mut stream = request.into_inner();
        let (tx, rx) = mpsc::unbounded_channel();
        let registry = self.registry.clone();
        let local_node_id = self.local_node_id.clone();

        tokio::spawn(async move {
            while let Ok(Some(message)) = stream.message().await {
                Self::process_message(registry.clone(), local_node_id.clone(), message, tx.clone())
                    .await;
            }
        });

        Ok(Response::new(
            tokio_stream::wrappers::UnboundedReceiverStream::new(rx),
        ))
    }

    async fn get_connection_info(
        &self,
        _request: Request<ConnectionInfoRequestProto>,
    ) -> Result<Response<ConnectionInfoResponseProto>, Status> {
        let node_id = NodeIdProto {
            key: orbit_util::RngUtils::random_string(),
            namespace: "default".to_string(),
        };

        let response = ConnectionInfoResponseProto {
            node_id: Some(node_id),
        };

        Ok(Response::new(response))
    }
}
