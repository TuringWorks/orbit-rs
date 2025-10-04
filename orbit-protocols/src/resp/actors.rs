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
        for value in values.into_iter().rev() {
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
}
