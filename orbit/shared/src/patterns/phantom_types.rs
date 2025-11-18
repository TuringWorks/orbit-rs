//! Phantom type patterns for compile-time guarantees
//!
//! Uses PhantomData to enforce invariants at compile time without runtime cost.

use std::marker::PhantomData;

// ===== Unit Types for Phantom Parameters =====

/// Marker types for measurement units
pub mod units {
    #[derive(Debug, Clone, Copy)]
    pub struct Meters;

    #[derive(Debug, Clone, Copy)]
    pub struct Feet;

    #[derive(Debug, Clone, Copy)]
    pub struct Kilometers;

    #[derive(Debug, Clone, Copy)]
    pub struct Miles;
}

/// Distance with compile-time unit tracking
#[derive(Debug, Clone, Copy)]
pub struct Distance<Unit> {
    value: f64,
    _unit: PhantomData<Unit>,
}

impl<Unit> Distance<Unit> {
    pub fn new(value: f64) -> Self {
        Self {
            value,
            _unit: PhantomData,
        }
    }

    pub fn value(&self) -> f64 {
        self.value
    }
}

// Unit conversions
impl Distance<units::Meters> {
    pub fn to_kilometers(self) -> Distance<units::Kilometers> {
        Distance::new(self.value / 1000.0)
    }

    pub fn to_feet(self) -> Distance<units::Feet> {
        Distance::new(self.value * 3.28084)
    }
}

impl Distance<units::Kilometers> {
    pub fn to_meters(self) -> Distance<units::Meters> {
        Distance::new(self.value * 1000.0)
    }

    pub fn to_miles(self) -> Distance<units::Miles> {
        Distance::new(self.value * 0.621371)
    }
}

impl Distance<units::Feet> {
    pub fn to_meters(self) -> Distance<units::Meters> {
        Distance::new(self.value / 3.28084)
    }
}

impl Distance<units::Miles> {
    pub fn to_kilometers(self) -> Distance<units::Kilometers> {
        Distance::new(self.value / 0.621371)
    }
}

// ===== Branded Types Pattern =====

/// Phantom type for branding identifiers
pub mod brands {
    #[derive(Debug, Clone, Copy)]
    pub struct User;

    #[derive(Debug, Clone, Copy)]
    pub struct Session;

    #[derive(Debug, Clone, Copy)]
    pub struct Transaction;

    #[derive(Debug, Clone, Copy)]
    pub struct Resource;
}

/// Type-safe identifier with brand
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id<Brand> {
    value: String,
    _brand: PhantomData<Brand>,
}

impl<Brand> Id<Brand> {
    pub fn new(value: String) -> Self {
        Self {
            value,
            _brand: PhantomData,
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Type aliases for common IDs
pub type UserId = Id<brands::User>;
pub type SessionId = Id<brands::Session>;
pub type TransactionId = Id<brands::Transaction>;
pub type ResourceId = Id<brands::Resource>;

// These prevent mixing up different ID types:
// let user_id: UserId = UserId::new("user-123".to_string());
// let session_id: SessionId = user_id; // Compile error!

// ===== State Machine with Phantom Types =====

/// Payment states
pub mod payment_states {
    #[derive(Debug, Clone, Copy)]
    pub struct Pending;

    #[derive(Debug, Clone, Copy)]
    pub struct Authorized;

    #[derive(Debug, Clone, Copy)]
    pub struct Captured;

    #[derive(Debug, Clone, Copy)]
    pub struct Refunded;
}

/// Payment with type-level state tracking
#[derive(Debug)]
pub struct Payment<State> {
    id: String,
    amount: f64,
    _state: PhantomData<State>,
}

impl Payment<payment_states::Pending> {
    pub fn new(id: String, amount: f64) -> Self {
        Self {
            id,
            amount,
            _state: PhantomData,
        }
    }

    pub fn authorize(self, auth_code: String) -> Payment<payment_states::Authorized> {
        println!("Authorizing payment {} with code {}", self.id, auth_code);
        Payment {
            id: self.id,
            amount: self.amount,
            _state: PhantomData,
        }
    }

    pub fn cancel(self) -> String {
        format!("Payment {} cancelled", self.id)
    }
}

impl Payment<payment_states::Authorized> {
    pub fn capture(self) -> Payment<payment_states::Captured> {
        println!("Capturing payment {}", self.id);
        Payment {
            id: self.id,
            amount: self.amount,
            _state: PhantomData,
        }
    }

    pub fn void(self) -> Payment<payment_states::Pending> {
        println!("Voiding authorization for payment {}", self.id);
        Payment {
            id: self.id,
            amount: self.amount,
            _state: PhantomData,
        }
    }
}

impl Payment<payment_states::Captured> {
    pub fn refund(self, amount: f64) -> Payment<payment_states::Refunded> {
        println!("Refunding {} from payment {}", amount, self.id);
        Payment {
            id: self.id,
            amount,
            _state: PhantomData,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl Payment<payment_states::Refunded> {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn refunded_amount(&self) -> f64 {
        self.amount
    }
}

// ===== Variance Markers =====

/// Phantom type for covariance
pub struct Covariant<T> {
    _marker: PhantomData<fn() -> T>,
}

/// Phantom type for contravariance
pub struct Contravariant<T> {
    _marker: PhantomData<fn(T)>,
}

/// Phantom type for invariance
pub struct Invariant<T> {
    _marker: PhantomData<fn(T) -> T>,
}

// ===== Compile-Time Validation =====

/// Marker for validated data
pub struct Validated;

/// Marker for unvalidated data
pub struct Unvalidated;

/// Data that may or may not be validated
#[derive(Debug, Clone)]
pub struct Data<ValidationState> {
    content: String,
    _validation: PhantomData<ValidationState>,
}

impl Data<Unvalidated> {
    pub fn new(content: String) -> Self {
        Self {
            content,
            _validation: PhantomData,
        }
    }

    pub fn validate(self) -> Result<Data<Validated>, String> {
        if self.content.is_empty() {
            Err("Data cannot be empty".to_string())
        } else if self.content.len() > 1000 {
            Err("Data too large".to_string())
        } else {
            Ok(Data {
                content: self.content,
                _validation: PhantomData,
            })
        }
    }
}

impl Data<Validated> {
    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn process(&self) -> String {
        format!("Processing validated data: {}", self.content)
    }
}

// ===== Capability-Based Security =====

/// Permission markers
pub mod permissions {
    #[derive(Debug, Clone, Copy)]
    pub struct Read;

    #[derive(Debug, Clone, Copy)]
    pub struct Write;

    #[derive(Debug, Clone, Copy)]
    pub struct Execute;
}

/// File handle with compile-time permission tracking
pub struct FileHandle<Permission> {
    path: String,
    _permission: PhantomData<Permission>,
}

impl FileHandle<permissions::Read> {
    pub fn open_read(path: String) -> Self {
        Self {
            path,
            _permission: PhantomData,
        }
    }

    pub fn read(&self) -> String {
        format!("Reading from {}", self.path)
    }
}

impl FileHandle<permissions::Write> {
    pub fn open_write(path: String) -> Self {
        Self {
            path,
            _permission: PhantomData,
        }
    }

    pub fn write(&self, data: &str) -> String {
        format!("Writing '{}' to {}", data, self.path)
    }
}

impl FileHandle<permissions::Execute> {
    pub fn open_execute(path: String) -> Self {
        Self {
            path,
            _permission: PhantomData,
        }
    }

    pub fn execute(&self) -> String {
        format!("Executing {}", self.path)
    }
}

// ===== Zero-Sized Type Markers =====

/// Marker trait for sorted collections
pub trait Sorted {}

/// Marker trait for unsorted collections
pub trait Unsorted {}

/// Collection with sort state tracked at compile time
pub struct Collection<T, SortState> {
    items: Vec<T>,
    _sort_state: PhantomData<SortState>,
}

impl<T> Collection<T, Unsorted> {
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            _sort_state: PhantomData,
        }
    }

    pub fn sort(mut self) -> Collection<T, Sorted>
    where
        T: Ord,
    {
        self.items.sort();
        Collection {
            items: self.items,
            _sort_state: PhantomData,
        }
    }
}

impl<T> Collection<T, Sorted> {
    /// Binary search only available on sorted collections
    pub fn binary_search(&self, item: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        self.items.binary_search(item)
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }
}

impl<T, S> Collection<T, S> {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_units() {
        let meters = Distance::<units::Meters>::new(1000.0);
        let kilometers = meters.to_kilometers();
        assert!((kilometers.value() - 1.0).abs() < 0.001);

        let feet = Distance::<units::Meters>::new(10.0).to_feet();
        assert!((feet.value() - 32.8084).abs() < 0.001);
    }

    #[test]
    fn test_branded_ids() {
        let user_id = UserId::new("user-123".to_string());
        let session_id = SessionId::new("session-456".to_string());

        assert_eq!(user_id.value(), "user-123");
        assert_eq!(session_id.value(), "session-456");

        // These IDs have different types and cannot be confused
        assert_ne!(
            std::mem::discriminant(&user_id),
            std::mem::discriminant(&session_id)
        );
    }

    #[test]
    fn test_payment_state_machine() {
        let payment = Payment::new("pay-123".to_string(), 100.0);

        let authorized = payment.authorize("AUTH123".to_string());
        let captured = authorized.capture();

        assert_eq!(captured.id(), "pay-123");

        let refunded = captured.refund(50.0);
        assert_eq!(refunded.refunded_amount(), 50.0);
    }

    #[test]
    fn test_validation_state() {
        let unvalidated = Data::new("test data".to_string());
        let validated = unvalidated.validate().unwrap();

        assert_eq!(validated.content(), "test data");
        assert_eq!(validated.process(), "Processing validated data: test data");

        let invalid = Data::new("".to_string());
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_file_permissions() {
        let read_handle = FileHandle::open_read("/path/to/file".to_string());
        assert!(read_handle.read().contains("Reading"));

        let write_handle = FileHandle::open_write("/path/to/file".to_string());
        assert!(write_handle.write("data").contains("Writing"));

        let exec_handle = FileHandle::open_execute("/path/to/script".to_string());
        assert!(exec_handle.execute().contains("Executing"));

        // Compile error: read_handle.write("data");
        // Compile error: write_handle.execute();
    }

    #[test]
    fn test_sorted_collection() {
        let unsorted = Collection::new(vec![5, 2, 8, 1, 9]);
        let sorted = unsorted.sort();

        assert_eq!(sorted.items(), &[1, 2, 5, 8, 9]);
        assert_eq!(sorted.binary_search(&5), Ok(2));
        assert_eq!(sorted.len(), 5);

        // Compile error on unsorted collection:
        // unsorted.binary_search(&5);
    }

    #[test]
    fn test_payment_invalid_transitions() {
        let payment = Payment::new("pay-456".to_string(), 200.0);
        let cancelled = payment.cancel();
        assert!(cancelled.contains("cancelled"));

        // These would be compile errors:
        // payment.capture(); // Can't capture without authorization
        // authorized.refund(50.0); // Can't refund without capturing
    }
}
