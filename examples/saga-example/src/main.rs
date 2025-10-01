use async_trait::async_trait;
use orbit_shared::exception::{OrbitError, OrbitResult};
use orbit_shared::mesh::NodeId;
use orbit_shared::saga::{
    SagaConfig, SagaContext, SagaDefinition, SagaEventHandler, SagaExecution, SagaOrchestrator,
    SagaStep, SagaStepMetadata, StepResult,
};
use orbit_shared::transaction_log::{PersistentLogConfig, SqliteTransactionLogger};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};
#[allow(unused_imports)]
use tracing::debug;

/// Order data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub order_id: String,
    pub customer_id: String,
    pub items: Vec<OrderItem>,
    pub total_amount: f64,
    pub payment_method: String,
    pub shipping_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub product_id: String,
    pub quantity: u32,
    pub price: f64,
}

/// Payment processing step
pub struct PaymentProcessingStep {
    metadata: SagaStepMetadata,
}

impl PaymentProcessingStep {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            metadata: SagaStepMetadata {
                step_id: "payment_processing".to_string(),
                step_name: "Payment Processing".to_string(),
                description: Some("Process payment for the order".to_string()),
                timeout: Some(Duration::from_secs(30)),
                max_retries: Some(3),
                parallel_eligible: false,
                dependencies: vec![],
                tags: {
                    let mut tags = HashMap::new();
                    tags.insert("category".to_string(), "payment".to_string());
                    tags.insert("critical".to_string(), "true".to_string());
                    tags
                },
            },
        }
    }

    async fn charge_payment(&self, order: &Order) -> OrbitResult<String> {
        info!(
            "Processing payment for order {} - Amount: ${}",
            order.order_id, order.total_amount
        );

        // Simulate payment processing
        sleep(Duration::from_millis(500)).await;

        // Simulate payment failure for demonstration (10% chance)
        if order.order_id.ends_with('5') {
            return Err(OrbitError::internal("Payment failed: Insufficient funds"));
        }

        let payment_id = format!(
            "payment_{}",
            uuid::Uuid::new_v4().to_string()[..8].to_uppercase()
        );
        info!("Payment processed successfully: {}", payment_id);

        Ok(payment_id)
    }

    async fn refund_payment(&self, payment_id: &str, amount: f64) -> OrbitResult<()> {
        info!("Refunding payment {} - Amount: ${}", payment_id, amount);

        // Simulate refund processing
        sleep(Duration::from_millis(300)).await;

        info!("Payment refunded successfully: {}", payment_id);
        Ok(())
    }
}

#[async_trait]
impl SagaStep for PaymentProcessingStep {
    async fn execute(&self, context: &SagaContext) -> OrbitResult<StepResult> {
        let order: Order = context
            .get_typed_data("order")?
            .ok_or_else(|| OrbitError::internal("Order data not found in context"))?;

        match self.charge_payment(&order).await {
            Ok(payment_id) => {
                // Store payment ID in context for compensation
                let mut updated_context = context.clone();
                updated_context.set_data("payment_id".to_string(), serde_json::json!(payment_id));

                info!(
                    "Payment step completed successfully for order {}",
                    order.order_id
                );
                Ok(StepResult::Success)
            }
            Err(e) => {
                error!("Payment step failed for order {}: {}", order.order_id, e);
                Ok(StepResult::PermanentFailure(e.to_string()))
            }
        }
    }

    async fn compensate(&self, context: &SagaContext) -> OrbitResult<StepResult> {
        let payment_id: Option<String> = context.get_typed_data("payment_id")?;
        let order: Order = context
            .get_typed_data("order")?
            .ok_or_else(|| OrbitError::internal("Order data not found in context"))?;

        if let Some(payment_id) = payment_id {
            match self.refund_payment(&payment_id, order.total_amount).await {
                Ok(()) => {
                    info!(
                        "Payment compensation completed for order {}",
                        order.order_id
                    );
                    Ok(StepResult::Success)
                }
                Err(e) => {
                    error!(
                        "Payment compensation failed for order {}: {}",
                        order.order_id, e
                    );
                    Err(e)
                }
            }
        } else {
            info!("No payment to compensate for order {}", order.order_id);
            Ok(StepResult::Success)
        }
    }

    fn metadata(&self) -> &SagaStepMetadata {
        &self.metadata
    }
}

/// Inventory reservation step
pub struct InventoryReservationStep {
    metadata: SagaStepMetadata,
}

impl InventoryReservationStep {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            metadata: SagaStepMetadata {
                step_id: "inventory_reservation".to_string(),
                step_name: "Inventory Reservation".to_string(),
                description: Some("Reserve inventory for order items".to_string()),
                timeout: Some(Duration::from_secs(15)),
                max_retries: Some(2),
                parallel_eligible: false,
                dependencies: vec!["payment_processing".to_string()],
                tags: {
                    let mut tags = HashMap::new();
                    tags.insert("category".to_string(), "inventory".to_string());
                    tags
                },
            },
        }
    }

    async fn reserve_inventory(&self, order: &Order) -> OrbitResult<Vec<String>> {
        info!("Reserving inventory for order {}", order.order_id);

        let mut reservation_ids = Vec::new();

        for item in &order.items {
            info!(
                "Reserving {} units of product {}",
                item.quantity, item.product_id
            );

            // Simulate inventory check and reservation
            sleep(Duration::from_millis(100)).await;

            // Simulate inventory shortage for demonstration
            if item.product_id == "PRODUCT_999" {
                return Err(OrbitError::internal("Insufficient inventory"));
            }

            let reservation_id = format!(
                "res_{}_{}",
                item.product_id,
                uuid::Uuid::new_v4().to_string()[..6].to_uppercase()
            );
            reservation_ids.push(reservation_id.clone());

            info!("Reserved inventory: {}", reservation_id);
        }

        Ok(reservation_ids)
    }

    async fn release_inventory(&self, reservation_ids: &[String]) -> OrbitResult<()> {
        info!("Releasing inventory reservations: {:?}", reservation_ids);

        for reservation_id in reservation_ids {
            sleep(Duration::from_millis(50)).await;
            info!("Released inventory reservation: {}", reservation_id);
        }

        Ok(())
    }
}

#[async_trait]
impl SagaStep for InventoryReservationStep {
    async fn execute(&self, context: &SagaContext) -> OrbitResult<StepResult> {
        let order: Order = context
            .get_typed_data("order")?
            .ok_or_else(|| OrbitError::internal("Order data not found in context"))?;

        match self.reserve_inventory(&order).await {
            Ok(reservation_ids) => {
                let mut updated_context = context.clone();
                updated_context.set_data(
                    "reservation_ids".to_string(),
                    serde_json::json!(reservation_ids),
                );

                info!(
                    "Inventory reservation step completed successfully for order {}",
                    order.order_id
                );
                Ok(StepResult::Success)
            }
            Err(e) => {
                error!(
                    "Inventory reservation step failed for order {}: {}",
                    order.order_id, e
                );
                Ok(StepResult::PermanentFailure(e.to_string()))
            }
        }
    }

    async fn compensate(&self, context: &SagaContext) -> OrbitResult<StepResult> {
        let reservation_ids: Option<Vec<String>> = context.get_typed_data("reservation_ids")?;
        let order: Order = context
            .get_typed_data("order")?
            .ok_or_else(|| OrbitError::internal("Order data not found in context"))?;

        if let Some(reservation_ids) = reservation_ids {
            if !reservation_ids.is_empty() {
                match self.release_inventory(&reservation_ids).await {
                    Ok(()) => {
                        info!(
                            "Inventory compensation completed for order {}",
                            order.order_id
                        );
                        Ok(StepResult::Success)
                    }
                    Err(e) => {
                        error!(
                            "Inventory compensation failed for order {}: {}",
                            order.order_id, e
                        );
                        Err(e)
                    }
                }
            } else {
                info!("No inventory to compensate for order {}", order.order_id);
                Ok(StepResult::Success)
            }
        } else {
            info!(
                "No inventory reservations to compensate for order {}",
                order.order_id
            );
            Ok(StepResult::Success)
        }
    }

    fn metadata(&self) -> &SagaStepMetadata {
        &self.metadata
    }
}

/// Shipping arrangement step
pub struct ShippingArrangementStep {
    metadata: SagaStepMetadata,
}

impl ShippingArrangementStep {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            metadata: SagaStepMetadata {
                step_id: "shipping_arrangement".to_string(),
                step_name: "Shipping Arrangement".to_string(),
                description: Some("Arrange shipping for the order".to_string()),
                timeout: Some(Duration::from_secs(20)),
                max_retries: Some(3),
                parallel_eligible: false,
                dependencies: vec!["inventory_reservation".to_string()],
                tags: {
                    let mut tags = HashMap::new();
                    tags.insert("category".to_string(), "shipping".to_string());
                    tags
                },
            },
        }
    }

    async fn arrange_shipping(&self, order: &Order) -> OrbitResult<String> {
        info!(
            "Arranging shipping for order {} to {}",
            order.order_id, order.shipping_address
        );

        // Simulate shipping arrangement
        sleep(Duration::from_millis(300)).await;

        let tracking_number = format!(
            "TRACK_{}",
            uuid::Uuid::new_v4().to_string()[..10].to_uppercase()
        );
        info!(
            "Shipping arranged with tracking number: {}",
            tracking_number
        );

        Ok(tracking_number)
    }

    async fn cancel_shipping(&self, tracking_number: &str) -> OrbitResult<()> {
        info!(
            "Cancelling shipping with tracking number: {}",
            tracking_number
        );

        // Simulate shipping cancellation
        sleep(Duration::from_millis(200)).await;

        info!("Shipping cancelled successfully: {}", tracking_number);
        Ok(())
    }
}

#[async_trait]
impl SagaStep for ShippingArrangementStep {
    async fn execute(&self, context: &SagaContext) -> OrbitResult<StepResult> {
        let order: Order = context
            .get_typed_data("order")?
            .ok_or_else(|| OrbitError::internal("Order data not found in context"))?;

        match self.arrange_shipping(&order).await {
            Ok(tracking_number) => {
                let mut updated_context = context.clone();
                updated_context.set_data(
                    "tracking_number".to_string(),
                    serde_json::json!(tracking_number),
                );

                info!(
                    "Shipping arrangement step completed successfully for order {}",
                    order.order_id
                );
                Ok(StepResult::Success)
            }
            Err(e) => {
                error!(
                    "Shipping arrangement step failed for order {}: {}",
                    order.order_id, e
                );
                Ok(StepResult::RetryableFailure(e.to_string()))
            }
        }
    }

    async fn compensate(&self, context: &SagaContext) -> OrbitResult<StepResult> {
        let tracking_number: Option<String> = context.get_typed_data("tracking_number")?;
        let order: Order = context
            .get_typed_data("order")?
            .ok_or_else(|| OrbitError::internal("Order data not found in context"))?;

        if let Some(tracking_number) = tracking_number {
            match self.cancel_shipping(&tracking_number).await {
                Ok(()) => {
                    info!(
                        "Shipping compensation completed for order {}",
                        order.order_id
                    );
                    Ok(StepResult::Success)
                }
                Err(e) => {
                    error!(
                        "Shipping compensation failed for order {}: {}",
                        order.order_id, e
                    );
                    Err(e)
                }
            }
        } else {
            info!("No shipping to compensate for order {}", order.order_id);
            Ok(StepResult::Success)
        }
    }

    fn metadata(&self) -> &SagaStepMetadata {
        &self.metadata
    }
}

/// Order confirmation step
pub struct OrderConfirmationStep {
    metadata: SagaStepMetadata,
}

impl OrderConfirmationStep {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            metadata: SagaStepMetadata {
                step_id: "order_confirmation".to_string(),
                step_name: "Order Confirmation".to_string(),
                description: Some("Send order confirmation to customer".to_string()),
                timeout: Some(Duration::from_secs(10)),
                max_retries: Some(2),
                parallel_eligible: false,
                dependencies: vec!["shipping_arrangement".to_string()],
                tags: {
                    let mut tags = HashMap::new();
                    tags.insert("category".to_string(), "notification".to_string());
                    tags
                },
            },
        }
    }

    async fn send_confirmation(&self, order: &Order, tracking_number: &str) -> OrbitResult<()> {
        info!(
            "Sending order confirmation to customer {} for order {}",
            order.customer_id, order.order_id
        );

        // Simulate sending confirmation email
        sleep(Duration::from_millis(150)).await;

        info!(
            "Order confirmation sent with tracking number: {}",
            tracking_number
        );
        Ok(())
    }

    async fn send_cancellation_notice(&self, order: &Order) -> OrbitResult<()> {
        info!(
            "Sending order cancellation notice to customer {} for order {}",
            order.customer_id, order.order_id
        );

        // Simulate sending cancellation email
        sleep(Duration::from_millis(100)).await;

        info!(
            "Order cancellation notice sent for order {}",
            order.order_id
        );
        Ok(())
    }
}

#[async_trait]
impl SagaStep for OrderConfirmationStep {
    async fn execute(&self, context: &SagaContext) -> OrbitResult<StepResult> {
        let order: Order = context
            .get_typed_data("order")?
            .ok_or_else(|| OrbitError::internal("Order data not found in context"))?;

        let tracking_number: String = context
            .get_typed_data("tracking_number")?
            .ok_or_else(|| OrbitError::internal("Tracking number not found in context"))?;

        match self.send_confirmation(&order, &tracking_number).await {
            Ok(()) => {
                info!(
                    "Order confirmation step completed successfully for order {}",
                    order.order_id
                );
                Ok(StepResult::Success)
            }
            Err(e) => {
                error!(
                    "Order confirmation step failed for order {}: {}",
                    order.order_id, e
                );
                Ok(StepResult::RetryableFailure(e.to_string()))
            }
        }
    }

    async fn compensate(&self, context: &SagaContext) -> OrbitResult<StepResult> {
        let order: Order = context
            .get_typed_data("order")?
            .ok_or_else(|| OrbitError::internal("Order data not found in context"))?;

        match self.send_cancellation_notice(&order).await {
            Ok(()) => {
                info!(
                    "Order confirmation compensation completed for order {}",
                    order.order_id
                );
                Ok(StepResult::Success)
            }
            Err(e) => {
                error!(
                    "Order confirmation compensation failed for order {}: {}",
                    order.order_id, e
                );
                Err(e)
            }
        }
    }

    fn metadata(&self) -> &SagaStepMetadata {
        &self.metadata
    }
}

/// Event handler for logging saga events
pub struct OrderSagaEventHandler;

#[async_trait]
impl SagaEventHandler for OrderSagaEventHandler {
    async fn on_saga_started(&self, execution: &SagaExecution) -> OrbitResult<()> {
        info!("🚀 Order saga started: {}", execution.saga_id);
        Ok(())
    }

    async fn on_saga_completed(&self, execution: &SagaExecution) -> OrbitResult<()> {
        info!(
            "✅ Order saga completed successfully: {}",
            execution.saga_id
        );
        Ok(())
    }

    async fn on_saga_failed(&self, execution: &SagaExecution) -> OrbitResult<()> {
        error!(
            "❌ Order saga failed: {} - {}",
            execution.saga_id,
            execution
                .error_message
                .as_deref()
                .unwrap_or("Unknown error")
        );
        Ok(())
    }

    async fn on_step_started(&self, execution: &SagaExecution, step_id: &str) -> OrbitResult<()> {
        info!("🔄 Step started: {} in saga {}", step_id, execution.saga_id);
        Ok(())
    }

    async fn on_step_completed(
        &self,
        execution: &SagaExecution,
        step_id: &str,
        result: &StepResult,
    ) -> OrbitResult<()> {
        match result {
            StepResult::Success => {
                info!(
                    "✓ Step completed: {} in saga {}",
                    step_id, execution.saga_id
                );
            }
            _ => {
                warn!(
                    "⚠ Step completed with result: {:?} - {} in saga {}",
                    result, step_id, execution.saga_id
                );
            }
        }
        Ok(())
    }

    async fn on_compensation_started(&self, execution: &SagaExecution) -> OrbitResult<()> {
        warn!("🔙 Compensation started for saga: {}", execution.saga_id);
        Ok(())
    }

    async fn on_compensation_completed(&self, execution: &SagaExecution) -> OrbitResult<()> {
        info!("↩️ Compensation completed for saga: {}", execution.saga_id);
        Ok(())
    }
}

async fn create_order_processing_saga() -> SagaDefinition {
    let config = SagaConfig {
        max_execution_time: Duration::from_secs(300), // 5 minutes
        default_step_timeout: Duration::from_secs(30),
        max_retry_attempts: 3,
        retry_backoff_initial: Duration::from_millis(200),
        retry_backoff_multiplier: 2.0,
        enable_parallel_execution: false, // Sequential execution for this example
        checkpoint_interval: Duration::from_secs(10),
        max_compensation_time: Duration::from_secs(180),
    };

    SagaDefinition::new("order_processing_saga".to_string(), "1.0".to_string())
        .with_config(config)
        .add_step(Arc::new(PaymentProcessingStep::new()))
        .add_step(Arc::new(InventoryReservationStep::new()))
        .add_step(Arc::new(ShippingArrangementStep::new()))
        .add_step(Arc::new(OrderConfirmationStep::new()))
        .with_tag("domain".to_string(), "e-commerce".to_string())
        .with_tag("priority".to_string(), "high".to_string())
}

fn create_sample_order(order_id: String) -> Order {
    Order {
        order_id,
        customer_id: "CUST_12345".to_string(),
        items: vec![
            OrderItem {
                product_id: "PRODUCT_001".to_string(),
                quantity: 2,
                price: 29.99,
            },
            OrderItem {
                product_id: "PRODUCT_002".to_string(),
                quantity: 1,
                price: 149.99,
            },
        ],
        total_amount: 209.97,
        payment_method: "CREDIT_CARD".to_string(),
        shipping_address: "123 Main St, Springfield, IL 62701".to_string(),
    }
}

#[tokio::main]
async fn main() -> OrbitResult<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("🛍️  Starting E-commerce Order Processing Saga Example");

    // Set up database
    let db_path = std::env::temp_dir().join("saga_example.db");
    let log_config = PersistentLogConfig {
        database_path: db_path,
        ..Default::default()
    };

    // Create components
    let logger = Arc::new(SqliteTransactionLogger::new(log_config).await?);
    let node_id = NodeId::new("order-processor-1".to_string(), "e-commerce".to_string());
    let orchestrator = Arc::new(SagaOrchestrator::new(node_id, logger));

    // Register event handler
    orchestrator
        .add_event_handler(Arc::new(OrderSagaEventHandler))
        .await;

    // Register saga definition
    let saga_definition = create_order_processing_saga().await;
    orchestrator
        .register_saga_definition(saga_definition)
        .await?;

    info!("✅ Saga orchestrator initialized and saga definition registered");

    // Start orchestrator
    orchestrator.start().await?;

    // Example 1: Successful order processing
    info!("\n📦 Example 1: Processing successful order");
    let order1 = create_sample_order("ORDER_001".to_string());
    let mut initial_context = HashMap::new();
    initial_context.insert("order".to_string(), serde_json::to_value(&order1)?);

    let saga_id1 = orchestrator
        .start_saga("order_processing_saga", initial_context)
        .await?;
    info!("Started saga for successful order: {}", saga_id1);

    // Wait for completion
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Example 2: Order with payment failure (ends with '5' to trigger failure)
    info!("\n💳 Example 2: Processing order with payment failure");
    let order2 = create_sample_order("ORDER_005".to_string()); // Will fail payment
    let mut initial_context2 = HashMap::new();
    initial_context2.insert("order".to_string(), serde_json::to_value(&order2)?);

    let saga_id2 = orchestrator
        .start_saga("order_processing_saga", initial_context2)
        .await?;
    info!("Started saga for failing order: {}", saga_id2);

    // Wait for completion
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Example 3: Order with inventory shortage
    info!("\n📦 Example 3: Processing order with inventory shortage");
    let mut order3 = create_sample_order("ORDER_003".to_string());
    order3.items.push(OrderItem {
        product_id: "PRODUCT_999".to_string(), // Will fail inventory
        quantity: 1,
        price: 99.99,
    });
    order3.total_amount += 99.99;

    let mut initial_context3 = HashMap::new();
    initial_context3.insert("order".to_string(), serde_json::to_value(&order3)?);

    let saga_id3 = orchestrator
        .start_saga("order_processing_saga", initial_context3)
        .await?;
    info!("Started saga for inventory shortage order: {}", saga_id3);

    // Wait for completion
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Display statistics
    let stats = orchestrator.get_stats().await;
    info!("\n📊 Saga Execution Statistics:");
    info!("Total sagas: {}", stats.total_sagas);
    info!("Completed sagas: {}", stats.completed_sagas);
    info!("Failed sagas: {}", stats.failed_sagas);
    info!("Compensated sagas: {}", stats.compensated_sagas);
    info!("Active sagas: {}", stats.active_sagas);
    info!("Total steps executed: {}", stats.total_steps_executed);
    info!("Total steps compensated: {}", stats.total_steps_compensated);

    // Check execution results
    info!("\n🔍 Checking execution results:");

    if let Ok(Some(execution1)) = orchestrator.get_execution(&saga_id1).await {
        info!(
            "Saga 1 ({}): State = {:?}, Steps completed = {}",
            saga_id1,
            execution1.state,
            execution1.completed_steps.len()
        );
    }

    if let Ok(Some(execution2)) = orchestrator.get_execution(&saga_id2).await {
        info!(
            "Saga 2 ({}): State = {:?}, Steps compensated = {}",
            saga_id2,
            execution2.state,
            execution2.compensated_steps.len()
        );
    }

    if let Ok(Some(execution3)) = orchestrator.get_execution(&saga_id3).await {
        info!(
            "Saga 3 ({}): State = {:?}, Steps compensated = {}",
            saga_id3,
            execution3.state,
            execution3.compensated_steps.len()
        );
    }

    info!("\n✅ E-commerce Order Processing Saga Example completed!");

    Ok(())
}
