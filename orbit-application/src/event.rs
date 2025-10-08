use crate::{AppError, AppEvent};

#[derive(Debug)]
pub struct EventBus;

impl EventBus {
    pub fn new() -> Self {
        Self
    }

    pub async fn publish(&self, _event: AppEvent) -> Result<(), AppError> {
        Ok(())
    }
}
