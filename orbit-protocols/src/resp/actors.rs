//! Actor definitions for RESP protocol storage

use async_trait::async_trait;
use orbit_shared::addressable::{ActorWithStringKey, Addressable};
use orbit_shared::exception::OrbitResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Actor for storing key-value pairs (Redis STRING type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValueActor {
    pub value: Option<String>,
    pub expiration: Option<u64>, // Unix timestamp in seconds
}

impl KeyValueActor {
    pub fn new() -> Self {
        Self {
            value: None,
            expiration: None,
        }
    }

    pub fn with_value(value: String) -> Self {
        Self {
            value: Some(value),
            expiration: None,
        }
    }

    pub fn set_value(&mut self, value: String) {
        self.value = Some(value);
    }

    pub fn get_value(&self) -> Option<&String> {
        if self.is_expired() {
            None
        } else {
            self.value.as_ref()
        }
    }

    pub fn set_expiration(&mut self, seconds: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.expiration = Some(now + seconds);
    }

    pub fn get_ttl(&self) -> i64 {
        match self.expiration {
            None => -1, // No expiration
            Some(exp) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if exp > now {
                    (exp - now) as i64
                } else {
                    -2 // Expired
                }
            }
        }
    }

    pub fn is_expired(&self) -> bool {
        self.get_ttl() == -2
    }
}

impl Default for KeyValueActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Addressable for KeyValueActor {
    fn addressable_type() -> &'static str {
        "KeyValueActor"
    }
}

impl ActorWithStringKey for KeyValueActor {}

/// Async trait for KeyValueActor methods
#[async_trait]
pub trait KeyValueActorMethods: Addressable {
    async fn get_value(&self) -> OrbitResult<Option<String>>;
    async fn set_value(&mut self, value: String) -> OrbitResult<()>;
    async fn delete_value(&mut self) -> OrbitResult<bool>;
    async fn set_expiration(&mut self, seconds: u64) -> OrbitResult<()>;
    async fn get_ttl(&self) -> OrbitResult<i64>;
    async fn exists(&self) -> OrbitResult<bool>;
}

#[async_trait]
impl KeyValueActorMethods for KeyValueActor {
    async fn get_value(&self) -> OrbitResult<Option<String>> {
        Ok(if self.is_expired() {
            None
        } else {
            self.value.clone()
        })
    }

    async fn set_value(&mut self, value: String) -> OrbitResult<()> {
        self.value = Some(value);
        Ok(())
    }

    async fn delete_value(&mut self) -> OrbitResult<bool> {
        let existed = self.value.is_some();
        self.value = None;
        self.expiration = None;
        Ok(existed)
    }

    async fn set_expiration(&mut self, seconds: u64) -> OrbitResult<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.expiration = Some(now + seconds);
        Ok(())
    }

    async fn get_ttl(&self) -> OrbitResult<i64> {
        Ok(self.get_ttl())
    }

    async fn exists(&self) -> OrbitResult<bool> {
        Ok(self.value.is_some() && !self.is_expired())
    }
}

/// Actor for storing hash maps (Redis HASH type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashActor {
    pub fields: HashMap<String, String>,
}

impl HashActor {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }

    pub fn hset(&mut self, field: String, value: String) -> bool {
        self.fields.insert(field, value).is_none()
    }

    pub fn hget(&self, field: &str) -> Option<&String> {
        self.fields.get(field)
    }

    pub fn hdel(&mut self, field: &str) -> bool {
        self.fields.remove(field).is_some()
    }

    pub fn hexists(&self, field: &str) -> bool {
        self.fields.contains_key(field)
    }

    pub fn hkeys(&self) -> Vec<String> {
        self.fields.keys().cloned().collect()
    }

    pub fn hvals(&self) -> Vec<String> {
        self.fields.values().cloned().collect()
    }

    pub fn hgetall(&self) -> Vec<(String, String)> {
        self.fields
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn hlen(&self) -> usize {
        self.fields.len()
    }
}

impl Default for HashActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Addressable for HashActor {
    fn addressable_type() -> &'static str {
        "HashActor"
    }
}

impl ActorWithStringKey for HashActor {}

/// Async trait for HashActor methods
#[async_trait]
pub trait HashActorMethods: Addressable {
    async fn hget(&self, field: &str) -> OrbitResult<Option<String>>;
    async fn hset(&mut self, field: String, value: String) -> OrbitResult<bool>;
    async fn hdel(&mut self, field: &str) -> OrbitResult<bool>;
    async fn hexists(&self, field: &str) -> OrbitResult<bool>;
    async fn hkeys(&self) -> OrbitResult<Vec<String>>;
    async fn hvals(&self) -> OrbitResult<Vec<String>>;
    async fn hgetall(&self) -> OrbitResult<Vec<(String, String)>>;
    async fn hlen(&self) -> OrbitResult<usize>;
}

#[async_trait]
impl HashActorMethods for HashActor {
    async fn hget(&self, field: &str) -> OrbitResult<Option<String>> {
        Ok(self.hget(field).cloned())
    }

    async fn hset(&mut self, field: String, value: String) -> OrbitResult<bool> {
        Ok(self.hset(field, value))
    }

    async fn hdel(&mut self, field: &str) -> OrbitResult<bool> {
        Ok(self.hdel(field))
    }

    async fn hexists(&self, field: &str) -> OrbitResult<bool> {
        Ok(self.hexists(field))
    }

    async fn hkeys(&self) -> OrbitResult<Vec<String>> {
        Ok(self.hkeys())
    }

    async fn hvals(&self) -> OrbitResult<Vec<String>> {
        Ok(self.hvals())
    }

    async fn hgetall(&self) -> OrbitResult<Vec<(String, String)>> {
        Ok(self.hgetall())
    }

    async fn hlen(&self) -> OrbitResult<usize> {
        Ok(self.hlen())
    }
}

/// Actor for storing lists (Redis LIST type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListActor {
    pub items: Vec<String>,
}

impl ListActor {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn lpush(&mut self, values: Vec<String>) -> usize {
        for value in values {
            self.items.insert(0, value);
        }
        self.items.len()
    }

    pub fn rpush(&mut self, values: Vec<String>) -> usize {
        self.items.extend(values);
        self.items.len()
    }

    pub fn lpop(&mut self, count: usize) -> Vec<String> {
        let count = count.min(self.items.len());
        self.items.drain(0..count).collect()
    }

    pub fn rpop(&mut self, count: usize) -> Vec<String> {
        let count = count.min(self.items.len());
        let start = self.items.len() - count;
        self.items.drain(start..).rev().collect()
    }

    pub fn lrange(&self, start: i64, stop: i64) -> Vec<String> {
        let len = self.items.len() as i64;

        // Convert negative indices
        let start = if start < 0 {
            (len + start).max(0)
        } else {
            start.min(len)
        } as usize;

        let stop = if stop < 0 {
            (len + stop + 1).max(0)
        } else {
            (stop + 1).min(len)
        } as usize;

        if start >= self.items.len() || start >= stop {
            Vec::new()
        } else {
            self.items[start..stop].to_vec()
        }
    }

    pub fn llen(&self) -> usize {
        self.items.len()
    }

    pub fn lindex(&self, index: i64) -> Option<&String> {
        let len = self.items.len() as i64;
        let idx = if index < 0 {
            (len + index).max(0)
        } else {
            index
        };

        if idx >= 0 && (idx as usize) < self.items.len() {
            Some(&self.items[idx as usize])
        } else {
            None
        }
    }
}

impl Default for ListActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Addressable for ListActor {
    fn addressable_type() -> &'static str {
        "ListActor"
    }
}

impl ActorWithStringKey for ListActor {}

/// Async trait for ListActor methods
#[async_trait]
pub trait ListActorMethods: Addressable {
    async fn lpush(&mut self, values: Vec<String>) -> OrbitResult<usize>;
    async fn rpush(&mut self, values: Vec<String>) -> OrbitResult<usize>;
    async fn lpop(&mut self, count: usize) -> OrbitResult<Vec<String>>;
    async fn rpop(&mut self, count: usize) -> OrbitResult<Vec<String>>;
    async fn lrange(&self, start: i64, stop: i64) -> OrbitResult<Vec<String>>;
    async fn llen(&self) -> OrbitResult<usize>;
    async fn lindex(&self, index: i64) -> OrbitResult<Option<String>>;
}

#[async_trait]
impl ListActorMethods for ListActor {
    async fn lpush(&mut self, values: Vec<String>) -> OrbitResult<usize> {
        Ok(self.lpush(values))
    }

    async fn rpush(&mut self, values: Vec<String>) -> OrbitResult<usize> {
        Ok(self.rpush(values))
    }

    async fn lpop(&mut self, count: usize) -> OrbitResult<Vec<String>> {
        Ok(self.lpop(count))
    }

    async fn rpop(&mut self, count: usize) -> OrbitResult<Vec<String>> {
        Ok(self.rpop(count))
    }

    async fn lrange(&self, start: i64, stop: i64) -> OrbitResult<Vec<String>> {
        Ok(self.lrange(start, stop))
    }

    async fn llen(&self) -> OrbitResult<usize> {
        Ok(self.llen())
    }

    async fn lindex(&self, index: i64) -> OrbitResult<Option<String>> {
        Ok(self.lindex(index).cloned())
    }
}

/// Actor for managing pub/sub channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubActor {
    pub subscribers: Vec<String>, // Subscriber IDs
    pub message_count: u64,
}

impl PubSubActor {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
            message_count: 0,
        }
    }

    pub fn subscribe(&mut self, subscriber_id: String) {
        if !self.subscribers.contains(&subscriber_id) {
            self.subscribers.push(subscriber_id);
        }
    }

    pub fn unsubscribe(&mut self, subscriber_id: &str) -> bool {
        if let Some(pos) = self.subscribers.iter().position(|s| s == subscriber_id) {
            self.subscribers.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn publish(&mut self, _message: String) -> usize {
        self.message_count += 1;
        self.subscribers.len()
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
}

impl Default for PubSubActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Addressable for PubSubActor {
    fn addressable_type() -> &'static str {
        "PubSubActor"
    }
}

impl ActorWithStringKey for PubSubActor {}

/// Async trait for PubSubActor methods
#[async_trait]
pub trait PubSubActorMethods: Addressable {
    async fn subscribe(&mut self, subscriber_id: String) -> OrbitResult<()>;
    async fn unsubscribe(&mut self, subscriber_id: &str) -> OrbitResult<bool>;
    async fn publish(&mut self, message: String) -> OrbitResult<usize>;
    async fn subscriber_count(&self) -> OrbitResult<usize>;
}

#[async_trait]
impl PubSubActorMethods for PubSubActor {
    async fn subscribe(&mut self, subscriber_id: String) -> OrbitResult<()> {
        self.subscribe(subscriber_id);
        Ok(())
    }

    async fn unsubscribe(&mut self, subscriber_id: &str) -> OrbitResult<bool> {
        Ok(self.unsubscribe(subscriber_id))
    }

    async fn publish(&mut self, message: String) -> OrbitResult<usize> {
        Ok(self.publish(message))
    }

    async fn subscriber_count(&self) -> OrbitResult<usize> {
        Ok(self.subscriber_count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyvalue_actor() {
        let mut actor = KeyValueActor::new();
        assert_eq!(actor.get_value(), None);

        actor.set_value("test".to_string());
        assert_eq!(actor.get_value(), Some(&"test".to_string()));

        actor.set_expiration(10);
        assert!(actor.get_ttl() > 0 && actor.get_ttl() <= 10);
        assert!(!actor.is_expired());
    }

    #[test]
    fn test_keyvalue_actor_expiration() {
        let mut actor = KeyValueActor::new();
        actor.set_value("expire_me".to_string());
        
        // Test immediate expiration
        actor.set_expiration(0);
        assert!(actor.is_expired() || actor.get_ttl() == 0);
        
        // Reset for future expiration test
        actor.expiration = Some((chrono::Utc::now() - chrono::Duration::seconds(1)).timestamp() as u64);
        assert!(actor.is_expired());
    }

    #[test]
    fn test_keyvalue_actor_without_expiration() {
        let mut actor = KeyValueActor::new();
        actor.set_value("no_expire".to_string());
        
        assert!(!actor.is_expired());
        assert_eq!(actor.get_ttl(), -1); // Indicates no expiration
    }

    #[test]
    fn test_keyvalue_actor_addressable_trait() {
        assert_eq!(KeyValueActor::addressable_type(), "KeyValueActor");
        
        let actor = KeyValueActor::new();
        // Test that it implements the required traits
        let _: &dyn Addressable = &actor;
        let _: &dyn ActorWithStringKey = &actor;
    }

    #[test]
    fn test_keyvalue_actor_methods() {
        let mut actor = KeyValueActor::new();
        
        actor.set_value("test".to_string());
        
        let value = actor.get_value();
        assert_eq!(value, Some(&"test".to_string()));
        
        let ttl = actor.get_ttl();
        assert_eq!(ttl, -1);
        
        let expired = actor.is_expired();
        assert!(!expired);
    }

    #[test]
    fn test_hash_actor() {
        let mut actor = HashActor::new();
        assert_eq!(actor.hlen(), 0);

        assert!(actor.hset("field1".to_string(), "value1".to_string()));
        assert_eq!(actor.hget("field1"), Some(&"value1".to_string()));
        assert!(actor.hexists("field1"));

        let all = actor.hgetall();
        assert_eq!(all.len(), 1);

        assert!(actor.hdel("field1"));
        assert_eq!(actor.hlen(), 0);
    }

    #[test]
    fn test_hash_actor_comprehensive() {
        let mut actor = HashActor::new();
        
        // Test multiple fields
        assert!(actor.hset("field1".to_string(), "value1".to_string()));
        assert!(actor.hset("field2".to_string(), "value2".to_string()));
        assert!(!actor.hset("field1".to_string(), "updated_value1".to_string())); // Update existing
        
        assert_eq!(actor.hlen(), 2);
        assert_eq!(actor.hget("field1"), Some(&"updated_value1".to_string()));
        
        let all_fields = actor.hgetall();
        assert_eq!(all_fields.len(), 2);
        assert!(all_fields.iter().any(|(k, _)| k == "field1"));
        assert!(all_fields.iter().any(|(k, _)| k == "field2"));
        
        // Test non-existent field
        assert_eq!(actor.hget("nonexistent"), None);
        assert!(!actor.hexists("nonexistent"));
        assert!(!actor.hdel("nonexistent"));
        
        // Test deletion
        assert!(actor.hdel("field1"));
        assert_eq!(actor.hlen(), 1);
        assert!(!actor.hexists("field1"));
    }

    #[test]
    fn test_hash_actor_edge_cases() {
        let mut actor = HashActor::new();
        
        // Test empty values
        assert!(actor.hset("".to_string(), "".to_string()));
        assert_eq!(actor.hget(""), Some(&"".to_string()));
        
        // Test large values
        let large_field = "x".repeat(1000);
        let large_value = "y".repeat(10000);
        assert!(actor.hset(large_field.clone(), large_value.clone()));
        assert_eq!(actor.hget(&large_field), Some(&large_value));
        
        assert_eq!(actor.hlen(), 2); // Empty field + large field
    }

    #[test]
    fn test_hash_actor_methods() {
        let mut actor = HashActor::new();
        
        let set_result = actor.hset("field".to_string(), "value".to_string());
        assert!(set_result);
        
        let get_result = actor.hget("field");
        assert_eq!(get_result, Some(&"value".to_string()));
        
        let exists_result = actor.hexists("field");
        assert!(exists_result);
        
        let len_result = actor.hlen();
        assert_eq!(len_result, 1);
        
        let all_result = actor.hgetall();
        assert_eq!(all_result.len(), 1);
        
        let del_result = actor.hdel("field");
        assert!(del_result);
    }

    #[test]
    fn test_list_actor() {
        let mut actor = ListActor::new();
        assert_eq!(actor.llen(), 0);

        actor.lpush(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(actor.llen(), 2);

        actor.rpush(vec!["c".to_string()]);
        assert_eq!(actor.llen(), 3);

        let range = actor.lrange(0, -1);
        assert_eq!(range, vec!["b", "a", "c"]);

        let popped = actor.lpop(1);
        assert_eq!(popped, vec!["b"]);
        assert_eq!(actor.llen(), 2);
    }

    #[test]
    fn test_list_actor_comprehensive() {
        let mut actor = ListActor::new();
        
        // Test rpush and lpush
        let lpush_count = actor.lpush(vec!["first".to_string(), "second".to_string()]);
        assert_eq!(lpush_count, 2);
        
        let rpush_count = actor.rpush(vec!["third".to_string(), "fourth".to_string()]);
        assert_eq!(rpush_count, 4);
        
        // List should be: [second, first, third, fourth]
        assert_eq!(actor.llen(), 4);
        
        // Test lindex
        assert_eq!(actor.lindex(0), Some(&"second".to_string()));
        assert_eq!(actor.lindex(-1), Some(&"fourth".to_string()));
        assert_eq!(actor.lindex(10), None); // Out of bounds
        
        // Test lrange with different ranges
        assert_eq!(actor.lrange(0, 1), vec!["second", "first"]);
        assert_eq!(actor.lrange(-2, -1), vec!["third", "fourth"]);
        assert_eq!(actor.lrange(0, 100), vec!["second", "first", "third", "fourth"]); // Beyond bounds
        
        // Test rpop
        let rpop_result = actor.rpop(2);
        assert_eq!(rpop_result, vec!["fourth", "third"]);
        assert_eq!(actor.llen(), 2);
        
        // Test lpop with more than available
        let lpop_result = actor.lpop(5);
        assert_eq!(lpop_result, vec!["second", "first"]);
        assert_eq!(actor.llen(), 0);
    }

    #[test]
    fn test_list_actor_edge_cases() {
        let mut actor = ListActor::new();
        
        // Test operations on empty list
        assert_eq!(actor.lpop(1), Vec::<String>::new());
        assert_eq!(actor.rpop(1), Vec::<String>::new());
        assert_eq!(actor.lrange(0, -1), Vec::<String>::new());
        assert_eq!(actor.lindex(0), None);
        
        // Test with empty vectors
        assert_eq!(actor.lpush(vec![]), 0);
        assert_eq!(actor.rpush(vec![]), 0);
        
        // Test with single elements
        actor.lpush(vec!["single".to_string()]);
        assert_eq!(actor.llen(), 1);
        assert_eq!(actor.lindex(0), Some(&"single".to_string()));
        assert_eq!(actor.lindex(-1), Some(&"single".to_string()));
        
        // Test popping more than available
        let all_popped = actor.lpop(10);
        assert_eq!(all_popped, vec!["single"]);
        assert_eq!(actor.llen(), 0);
    }

    #[test]
    fn test_list_actor_methods() {
        let mut actor = ListActor::new();
        
        let lpush_result = actor.lpush(vec!["test".to_string()]);
        assert_eq!(lpush_result, 1);
        
        let rpush_result = actor.rpush(vec!["test2".to_string()]);
        assert_eq!(rpush_result, 2);
        
        let len_result = actor.llen();
        assert_eq!(len_result, 2);
        
        let range_result = actor.lrange(0, -1);
        assert_eq!(range_result, vec!["test", "test2"]);
        
        let index_result = actor.lindex(0);
        assert_eq!(index_result, Some(&"test".to_string()));
        
        let lpop_result = actor.lpop(1);
        assert_eq!(lpop_result, vec!["test"]);
        
        let rpop_result = actor.rpop(1);
        assert_eq!(rpop_result, vec!["test2"]);
    }

    #[test]
    fn test_pubsub_actor() {
        let mut actor = PubSubActor::new();
        assert_eq!(actor.subscriber_count(), 0);

        actor.subscribe("sub1".to_string());
        actor.subscribe("sub2".to_string());
        assert_eq!(actor.subscriber_count(), 2);

        let count = actor.publish("test message".to_string());
        assert_eq!(count, 2);
        assert_eq!(actor.message_count, 1);

        assert!(actor.unsubscribe("sub1"));
        assert_eq!(actor.subscriber_count(), 1);
    }

    #[test]
    fn test_pubsub_actor_comprehensive() {
        let mut actor = PubSubActor::new();
        
        // Test duplicate subscription
        actor.subscribe("sub1".to_string());
        actor.subscribe("sub1".to_string()); // Duplicate
        assert_eq!(actor.subscriber_count(), 1); // Should not add duplicate
        
        // Test multiple subscribers
        for i in 2..=10 {
            actor.subscribe(format!("sub{}", i));
        }
        assert_eq!(actor.subscriber_count(), 9);
        
        // Test publishing to multiple subscribers
        let delivered_count = actor.publish("broadcast message".to_string());
        assert_eq!(delivered_count, 9);
        assert_eq!(actor.message_count, 1);
        
        // Test unsubscribe non-existent subscriber
        assert!(!actor.unsubscribe("nonexistent"));
        assert_eq!(actor.subscriber_count(), 9);
        
        // Test unsubscribe existing subscriber
        assert!(actor.unsubscribe("sub5"));
        assert_eq!(actor.subscriber_count(), 8);
        
        // Test another message
        let delivered_count2 = actor.publish("another message".to_string());
        assert_eq!(delivered_count2, 8);
        assert_eq!(actor.message_count, 2);
    }

    #[test]
    fn test_pubsub_actor_edge_cases() {
        let mut actor = PubSubActor::new();
        
        // Test publishing with no subscribers
        let count = actor.publish("no one listening".to_string());
        assert_eq!(count, 0);
        assert_eq!(actor.message_count, 1);
        
        // Test empty subscriber ID
        actor.subscribe("".to_string());
        assert_eq!(actor.subscriber_count(), 1);
        assert!(actor.unsubscribe(""));
        assert_eq!(actor.subscriber_count(), 0);
        
        // Test large subscriber list
        for i in 0..1000 {
            actor.subscribe(format!("subscriber_{}", i));
        }
        assert_eq!(actor.subscriber_count(), 1000);
        
        let delivered = actor.publish("mass message".to_string());
        assert_eq!(delivered, 1000);
    }

    #[test]
    fn test_pubsub_actor_methods() {
        let mut actor = PubSubActor::new();
        
        actor.subscribe("sub".to_string());
        
        let count_result = actor.subscriber_count();
        assert_eq!(count_result, 1);
        
        let pub_result = actor.publish("message".to_string());
        assert_eq!(pub_result, 1);
        
        let unsub_result = actor.unsubscribe("sub");
        assert!(unsub_result);
        
        let final_count = actor.subscriber_count();
        assert_eq!(final_count, 0);
    }

    #[test]
    fn test_actor_defaults() {
        let kv_default = KeyValueActor::default();
        assert_eq!(kv_default.value, None);
        
        let hash_default = HashActor::default();
        assert_eq!(hash_default.fields.len(), 0);
        
        let list_default = ListActor::default();
        assert_eq!(list_default.items.len(), 0);
        
        let pubsub_default = PubSubActor::default();
        assert_eq!(pubsub_default.subscribers.len(), 0);
        assert_eq!(pubsub_default.message_count, 0);
    }

    #[test]
    fn test_actor_serialization() {
        // Test that all actors can be serialized and deserialized
        
        // KeyValueActor
        let mut kv = KeyValueActor::new();
        kv.set_value("serialize_me".to_string());
        let kv_json = serde_json::to_string(&kv).unwrap();
        let kv_deserialized: KeyValueActor = serde_json::from_str(&kv_json).unwrap();
        assert_eq!(kv.value, kv_deserialized.value);
        
        // HashActor
        let mut hash = HashActor::new();
        hash.hset("field".to_string(), "value".to_string());
        let hash_json = serde_json::to_string(&hash).unwrap();
        let hash_deserialized: HashActor = serde_json::from_str(&hash_json).unwrap();
        assert_eq!(hash.fields, hash_deserialized.fields);
        
        // ListActor
        let mut list = ListActor::new();
        list.lpush(vec!["item".to_string()]);
        let list_json = serde_json::to_string(&list).unwrap();
        let list_deserialized: ListActor = serde_json::from_str(&list_json).unwrap();
        assert_eq!(list.items, list_deserialized.items);
        
        // PubSubActor
        let mut pubsub = PubSubActor::new();
        pubsub.subscribe("test_sub".to_string());
        pubsub.publish("test_msg".to_string());
        let pubsub_json = serde_json::to_string(&pubsub).unwrap();
        let pubsub_deserialized: PubSubActor = serde_json::from_str(&pubsub_json).unwrap();
        assert_eq!(pubsub.subscribers, pubsub_deserialized.subscribers);
        assert_eq!(pubsub.message_count, pubsub_deserialized.message_count);
    }

    #[test]
    fn test_actor_debug_output() {
        let kv = KeyValueActor::new();
        let debug_str = format!("{:?}", kv);
        assert!(debug_str.contains("KeyValueActor"));
        
        let hash = HashActor::new();
        let debug_str = format!("{:?}", hash);
        assert!(debug_str.contains("HashActor"));
        
        let list = ListActor::new();
        let debug_str = format!("{:?}", list);
        assert!(debug_str.contains("ListActor"));
        
        let pubsub = PubSubActor::new();
        let debug_str = format!("{:?}", pubsub);
        assert!(debug_str.contains("PubSubActor"));
    }
}
