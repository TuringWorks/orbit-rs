use crate::AppError;

#[derive(Debug, Default)]
pub struct EventBus;

impl EventBus {
    pub fn new() -> Self {
        Self
    }

    pub async fn publish<T>(&self, _event: T) -> Result<(), AppError> {
        Ok(())
    }
}
