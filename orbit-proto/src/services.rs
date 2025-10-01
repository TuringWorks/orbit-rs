//! gRPC service implementations for Orbit

use crate::*;
use tonic::{Request, Response, Status, Streaming};
use tokio::sync::mpsc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Connection service implementation for handling message streams
#[derive(Debug, Default, Clone)]
pub struct OrbitConnectionService {
    connections: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<MessageProto>>>>,
}

impl OrbitConnectionService {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[tonic::async_trait]
impl connection_service_server::ConnectionService for OrbitConnectionService {
    type OpenStreamStream = tokio_stream::wrappers::UnboundedReceiverStream<Result<MessageProto, Status>>;

    async fn open_stream(
        &self,
        request: Request<Streaming<MessageProto>>,
    ) -> Result<Response<Self::OpenStreamStream>, Status> {
        let mut stream = request.into_inner();
        let (tx, rx) = mpsc::unbounded_channel();
        
        // Handle incoming messages
        tokio::spawn(async move {
            while let Some(message) = stream.message().await? {
                // Process the message here
                tracing::info!("Received message: {:?}", message.message_id);
                
                // Echo the message back for now (implement actual routing logic later)
                let response = MessageProto {
                    message_id: message.message_id,
                    source: message.source,
                    target: message.target,
                    content: message.content,
                    attempts: message.attempts + 1,
                };
                
                if tx.send(Ok(response)).is_err() {
                    break;
                }
            }
            Ok::<(), Status>(())
        });

        Ok(Response::new(tokio_stream::wrappers::UnboundedReceiverStream::new(rx)))
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

/// Health service implementation
#[derive(Debug, Default, Clone)]
pub struct OrbitHealthService {
    status: Arc<Mutex<health_check_response::ServingStatus>>,
}

impl OrbitHealthService {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(health_check_response::ServingStatus::Serving)),
        }
    }

    pub async fn set_serving_status(&self, status: health_check_response::ServingStatus) {
        let mut current_status = self.status.lock().await;
        *current_status = status;
    }
}

#[tonic::async_trait]
impl health_service_server::HealthService for OrbitHealthService {
    type WatchStream = tokio_stream::wrappers::UnboundedReceiverStream<Result<HealthCheckResponse, Status>>;

    async fn check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let status = *self.status.lock().await;
        let response = HealthCheckResponse {
            status: status as i32,
        };

        Ok(Response::new(response))
    }

    async fn watch(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        let (tx, rx) = mpsc::unbounded_channel();
        let status = *self.status.lock().await;

        // Send initial status
        let response = HealthCheckResponse {
            status: status as i32,
        };
        
        if tx.send(Ok(response)).is_err() {
            return Err(Status::internal("Failed to send initial health status"));
        }

        // In a real implementation, we would watch for status changes
        // For now, just send the current status and close
        
        Ok(Response::new(tokio_stream::wrappers::UnboundedReceiverStream::new(rx)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::health_service_server::HealthService;
    use tokio_test;

    #[tokio::test]
    async fn test_connection_service() {
        let service = OrbitConnectionService::new();
        assert!(service.connections.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_health_service() {
        let service = OrbitHealthService::new();
        let request = Request::new(HealthCheckRequest {
            service: "orbit".to_string(),
        });

        let response = service.check(request).await.unwrap();
        assert_eq!(response.into_inner().status, health_check_response::ServingStatus::Serving as i32);
    }
}