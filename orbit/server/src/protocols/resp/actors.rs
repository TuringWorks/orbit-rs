//! Actor definitions for RESP protocol storage

// Complex return types are intentional for actor operations
#![allow(clippy::type_complexity)]

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

    pub fn append_value(&mut self, value: &str) -> usize {
        match &mut self.value {
            Some(existing) => {
                existing.push_str(value);
                existing.len()
            }
            None => {
                self.value = Some(value.to_string());
                value.len()
            }
        }
    }

    pub fn get_range(&self, start: i64, end: i64) -> Option<String> {
        if let Some(value) = &self.value {
            if self.is_expired() {
                return None;
            }
            let len = value.len() as i64;
            let start_idx = if start < 0 {
                (len + start).max(0) as usize
            } else {
                (start as usize).min(value.len())
            };
            let end_idx = if end < 0 {
                (len + end + 1).max(0) as usize
            } else {
                ((end + 1) as usize).min(value.len())
            };
            if start_idx >= end_idx || start_idx >= value.len() {
                Some(String::new())
            } else {
                Some(value[start_idx..end_idx].to_string())
            }
        } else {
            None
        }
    }

    pub fn set_range(&mut self, offset: usize, value: &str) -> usize {
        let current = self.value.get_or_insert_with(String::new);
        if offset >= current.len() {
            // Pad with zeros if offset is beyond current length
            current.extend((current.len()..offset).map(|_| '\0'));
            current.push_str(value);
        } else {
            // Replace existing bytes
            let mut chars: Vec<char> = current.chars().collect();
            let new_chars: Vec<char> = value.chars().collect();
            for (i, &ch) in new_chars.iter().enumerate() {
                if offset + i < chars.len() {
                    chars[offset + i] = ch;
                } else {
                    chars.push(ch);
                }
            }
            *current = chars.into_iter().collect();
        }
        current.len()
    }

    pub fn get_and_set(&mut self, new_value: String) -> Option<String> {
        if self.is_expired() {
            self.value = Some(new_value);
            None
        } else {
            let old = self.value.clone();
            self.value = Some(new_value);
            old
        }
    }

    pub fn strlen(&self) -> usize {
        if self.is_expired() {
            0
        } else {
            self.value.as_ref().map(|s| s.len()).unwrap_or(0)
        }
    }

    pub fn persist(&mut self) -> bool {
        let had_expiration = self.expiration.is_some();
        self.expiration = None;
        had_expiration
    }

    pub fn set_pexpiration(&mut self, milliseconds: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.expiration = Some((now + milliseconds) / 1000);
    }

    pub fn get_pttl(&self) -> i64 {
        match self.expiration {
            None => -1, // No expiration
            Some(exp) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                let exp_ms = exp * 1000;
                if exp_ms > now {
                    (exp_ms - now) as i64
                } else {
                    -2 // Expired
                }
            }
        }
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
    async fn append_value(&mut self, value: String) -> OrbitResult<usize>;
    async fn get_range(&self, start: i64, end: i64) -> OrbitResult<Option<String>>;
    async fn set_range(&mut self, offset: usize, value: String) -> OrbitResult<usize>;
    async fn get_and_set(&mut self, new_value: String) -> OrbitResult<Option<String>>;
    async fn strlen(&self) -> OrbitResult<usize>;
    async fn persist(&mut self) -> OrbitResult<bool>;
    async fn set_pexpiration(&mut self, milliseconds: u64) -> OrbitResult<()>;
    async fn get_pttl(&self) -> OrbitResult<i64>;
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

    async fn append_value(&mut self, value: String) -> OrbitResult<usize> {
        Ok(self.append_value(&value))
    }

    async fn get_range(&self, start: i64, end: i64) -> OrbitResult<Option<String>> {
        Ok(self.get_range(start, end))
    }

    async fn set_range(&mut self, offset: usize, value: String) -> OrbitResult<usize> {
        Ok(self.set_range(offset, &value))
    }

    async fn get_and_set(&mut self, new_value: String) -> OrbitResult<Option<String>> {
        Ok(self.get_and_set(new_value))
    }

    async fn strlen(&self) -> OrbitResult<usize> {
        Ok(self.strlen())
    }

    async fn persist(&mut self) -> OrbitResult<bool> {
        Ok(self.persist())
    }

    async fn set_pexpiration(&mut self, milliseconds: u64) -> OrbitResult<()> {
        self.set_pexpiration(milliseconds);
        Ok(())
    }

    async fn get_pttl(&self) -> OrbitResult<i64> {
        Ok(self.get_pttl())
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

    pub fn hincrby(&mut self, field: String, increment: i64) -> Result<i64, String> {
        let current_value = self
            .fields
            .get(&field)
            .map(|v| v.parse::<i64>())
            .unwrap_or(Ok(0))
            .map_err(|_| "ERR hash value is not an integer".to_string())?;

        let new_value = current_value + increment;
        self.fields.insert(field, new_value.to_string());
        Ok(new_value)
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
    async fn hincrby(&mut self, field: String, increment: i64) -> OrbitResult<Result<i64, String>>;
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

    async fn hincrby(&mut self, field: String, increment: i64) -> OrbitResult<Result<i64, String>> {
        Ok(self.hincrby(field, increment))
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

    pub fn lset(&mut self, index: i64, value: String) -> bool {
        let len = self.items.len() as i64;
        let idx = if index < 0 {
            (len + index).max(0)
        } else {
            index
        };

        if idx >= 0 && (idx as usize) < self.items.len() {
            self.items[idx as usize] = value;
            true
        } else {
            false
        }
    }

    pub fn lrem(&mut self, count: i64, value: &str) -> usize {
        let mut removed = 0;

        if count == 0 {
            // Remove all occurrences
            let original_len = self.items.len();
            self.items.retain(|item| item != value);
            removed = original_len - self.items.len();
        } else if count > 0 {
            // Remove first N occurrences
            let mut to_remove = count as usize;
            let mut i = 0;
            while i < self.items.len() && to_remove > 0 {
                if self.items[i] == value {
                    self.items.remove(i);
                    to_remove -= 1;
                    removed += 1;
                } else {
                    i += 1;
                }
            }
        } else {
            // Remove last N occurrences (count is negative)
            let mut to_remove = (-count) as usize;
            let mut i = self.items.len();
            while i > 0 && to_remove > 0 {
                i -= 1;
                if self.items[i] == value {
                    self.items.remove(i);
                    to_remove -= 1;
                    removed += 1;
                }
            }
        }

        removed
    }

    pub fn ltrim(&mut self, start: i64, stop: i64) {
        let len = self.items.len() as i64;

        // Convert negative indices
        let start_idx = if start < 0 {
            (len + start).max(0)
        } else {
            start.min(len)
        } as usize;

        let stop_idx = if stop < 0 {
            (len + stop + 1).max(0)
        } else {
            (stop + 1).min(len)
        } as usize;

        if start_idx >= self.items.len() || start_idx >= stop_idx {
            // Clear the list if invalid range
            self.items.clear();
        } else {
            // Keep only the elements in the range [start_idx, stop_idx)
            let trimmed: Vec<String> = self.items[start_idx..stop_idx].to_vec();
            self.items = trimmed;
        }
    }

    pub fn linsert(&mut self, before_after: &str, pivot: &str, element: String) -> i64 {
        // Find the pivot element
        if let Some(pivot_index) = self.items.iter().position(|item| item == pivot) {
            match before_after.to_uppercase().as_str() {
                "BEFORE" => {
                    self.items.insert(pivot_index, element);
                    self.items.len() as i64
                }
                "AFTER" => {
                    self.items.insert(pivot_index + 1, element);
                    self.items.len() as i64
                }
                _ => -1, // Invalid before/after argument
            }
        } else {
            0 // Pivot not found
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
    async fn lset(&mut self, index: i64, value: String) -> OrbitResult<bool>;
    async fn lrem(&mut self, count: i64, value: &str) -> OrbitResult<usize>;
    async fn ltrim(&mut self, start: i64, stop: i64) -> OrbitResult<()>;
    async fn linsert(
        &mut self,
        before_after: &str,
        pivot: &str,
        element: String,
    ) -> OrbitResult<i64>;
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

    async fn lset(&mut self, index: i64, value: String) -> OrbitResult<bool> {
        Ok(self.lset(index, value))
    }

    async fn lrem(&mut self, count: i64, value: &str) -> OrbitResult<usize> {
        Ok(self.lrem(count, value))
    }

    async fn ltrim(&mut self, start: i64, stop: i64) -> OrbitResult<()> {
        self.ltrim(start, stop);
        Ok(())
    }

    async fn linsert(
        &mut self,
        before_after: &str,
        pivot: &str,
        element: String,
    ) -> OrbitResult<i64> {
        Ok(self.linsert(before_after, pivot, element))
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

/// Actor for storing sets (Redis SET type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetActor {
    pub members: std::collections::HashSet<String>,
}

impl SetActor {
    pub fn new() -> Self {
        Self {
            members: std::collections::HashSet::new(),
        }
    }

    pub fn sadd(&mut self, members: Vec<String>) -> usize {
        let mut added_count = 0;
        for member in members {
            if self.members.insert(member) {
                added_count += 1;
            }
        }
        added_count
    }

    pub fn srem(&mut self, members: Vec<String>) -> usize {
        let mut removed_count = 0;
        for member in members {
            if self.members.remove(&member) {
                removed_count += 1;
            }
        }
        removed_count
    }

    pub fn smembers(&self) -> Vec<String> {
        self.members.iter().cloned().collect()
    }

    pub fn scard(&self) -> usize {
        self.members.len()
    }

    pub fn sismember(&self, member: &str) -> bool {
        self.members.contains(member)
    }

    pub fn sunion(&self, other: &SetActor) -> Vec<String> {
        self.members.union(&other.members).cloned().collect()
    }

    pub fn sinter(&self, other: &SetActor) -> Vec<String> {
        self.members.intersection(&other.members).cloned().collect()
    }

    pub fn sdiff(&self, other: &SetActor) -> Vec<String> {
        self.members.difference(&other.members).cloned().collect()
    }
}

impl Default for SetActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Addressable for SetActor {
    fn addressable_type() -> &'static str {
        "SetActor"
    }
}

impl ActorWithStringKey for SetActor {}

/// Async trait for SetActor methods
#[async_trait]
pub trait SetActorMethods: Addressable {
    async fn sadd(&mut self, members: Vec<String>) -> OrbitResult<usize>;
    async fn srem(&mut self, members: Vec<String>) -> OrbitResult<usize>;
    async fn smembers(&self) -> OrbitResult<Vec<String>>;
    async fn scard(&self) -> OrbitResult<usize>;
    async fn sismember(&self, member: &str) -> OrbitResult<bool>;
}

#[async_trait]
impl SetActorMethods for SetActor {
    async fn sadd(&mut self, members: Vec<String>) -> OrbitResult<usize> {
        Ok(self.sadd(members))
    }

    async fn srem(&mut self, members: Vec<String>) -> OrbitResult<usize> {
        Ok(self.srem(members))
    }

    async fn smembers(&self) -> OrbitResult<Vec<String>> {
        Ok(self.smembers())
    }

    async fn scard(&self) -> OrbitResult<usize> {
        Ok(self.scard())
    }

    async fn sismember(&self, member: &str) -> OrbitResult<bool> {
        Ok(self.sismember(member))
    }
}

/// Entry in a Redis Stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEntry {
    /// Entry ID in format "timestamp-sequence"
    pub id: String,
    /// Fields and values for this entry
    pub fields: HashMap<String, String>,
}

/// Consumer group for a Redis Stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerGroup {
    /// Name of the consumer group
    pub name: String,
    /// Last delivered ID
    pub last_delivered_id: String,
    /// Pending entries per consumer (consumer_name -> entry_ids)
    pub pending: HashMap<String, Vec<String>>,
    /// Consumers in this group
    pub consumers: HashMap<String, StreamConsumer>,
}

/// Consumer within a consumer group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConsumer {
    /// Consumer name
    pub name: String,
    /// Pending entries for this consumer
    pub pending_count: usize,
    /// Last seen timestamp
    pub last_seen: u64,
}

/// Actor for storing streams (Redis STREAM type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamActor {
    /// Stream entries in order
    pub entries: Vec<StreamEntry>,
    /// Last generated ID (timestamp, sequence)
    pub last_id: (u64, u64),
    /// Consumer groups
    pub groups: HashMap<String, ConsumerGroup>,
    /// Maximum length (0 = unlimited)
    pub max_length: usize,
}

impl StreamActor {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            last_id: (0, 0),
            groups: HashMap::new(),
            max_length: 0,
        }
    }

    /// Generate a new entry ID
    fn generate_id(&mut self, explicit_id: Option<&str>) -> Result<String, String> {
        if let Some(id_str) = explicit_id {
            if id_str == "*" {
                // Auto-generate ID
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                if now > self.last_id.0 {
                    self.last_id = (now, 0);
                } else {
                    self.last_id.1 += 1;
                }
                Ok(format!("{}-{}", self.last_id.0, self.last_id.1))
            } else if id_str.contains('-') {
                // Parse explicit ID
                let parts: Vec<&str> = id_str.split('-').collect();
                if parts.len() != 2 {
                    return Err("ERR Invalid stream ID".to_string());
                }
                let timestamp: u64 = parts[0]
                    .parse()
                    .map_err(|_| "ERR Invalid stream ID".to_string())?;
                let seq_str = parts[1];

                let sequence: u64 = if seq_str == "*" {
                    // Auto-generate sequence for given timestamp
                    if timestamp == self.last_id.0 {
                        self.last_id.1 + 1
                    } else if timestamp > self.last_id.0 {
                        0
                    } else {
                        return Err(
                            "ERR The ID specified is equal or smaller than the target stream top item".to_string()
                        );
                    }
                } else {
                    seq_str
                        .parse()
                        .map_err(|_| "ERR Invalid stream ID".to_string())?
                };

                // Validate that new ID is greater than last
                if (timestamp < self.last_id.0
                    || (timestamp == self.last_id.0 && sequence <= self.last_id.1))
                    && !self.entries.is_empty()
                {
                    return Err(
                        "ERR The ID specified is equal or smaller than the target stream top item"
                            .to_string(),
                    );
                }

                self.last_id = (timestamp, sequence);
                Ok(format!("{}-{}", timestamp, sequence))
            } else {
                Err("ERR Invalid stream ID format".to_string())
            }
        } else {
            // Default to auto-generate
            self.generate_id(Some("*"))
        }
    }

    /// XADD - Add entry to stream
    pub fn xadd(
        &mut self,
        id: Option<&str>,
        fields: Vec<(String, String)>,
    ) -> Result<String, String> {
        let entry_id = self.generate_id(id)?;

        let entry = StreamEntry {
            id: entry_id.clone(),
            fields: fields.into_iter().collect(),
        };

        self.entries.push(entry);

        // Trim if max_length is set
        if self.max_length > 0 && self.entries.len() > self.max_length {
            self.entries.drain(0..self.entries.len() - self.max_length);
        }

        Ok(entry_id)
    }

    /// XLEN - Get stream length
    pub fn xlen(&self) -> usize {
        self.entries.len()
    }

    /// Parse an ID string into (timestamp, sequence) for comparison
    fn parse_id(id: &str) -> Option<(u64, u64)> {
        if id == "-" {
            return Some((0, 0));
        }
        if id == "+" {
            return Some((u64::MAX, u64::MAX));
        }
        let parts: Vec<&str> = id.split('-').collect();
        if parts.len() == 2 {
            let ts = parts[0].parse().ok()?;
            let seq = parts[1].parse().ok()?;
            Some((ts, seq))
        } else if parts.len() == 1 {
            let ts = parts[0].parse().ok()?;
            Some((ts, 0))
        } else {
            None
        }
    }

    /// Compare two entry IDs
    #[allow(dead_code)]
    fn compare_ids(a: &str, b: &str) -> std::cmp::Ordering {
        let a_parsed = Self::parse_id(a).unwrap_or((0, 0));
        let b_parsed = Self::parse_id(b).unwrap_or((0, 0));
        a_parsed.cmp(&b_parsed)
    }

    /// XRANGE - Get entries in a range
    pub fn xrange(&self, start: &str, end: &str, count: Option<usize>) -> Vec<StreamEntry> {
        let start_parsed = Self::parse_id(start).unwrap_or((0, 0));
        let end_parsed = Self::parse_id(end).unwrap_or((u64::MAX, u64::MAX));

        let mut result: Vec<StreamEntry> = self
            .entries
            .iter()
            .filter(|entry| {
                if let Some(entry_id) = Self::parse_id(&entry.id) {
                    entry_id >= start_parsed && entry_id <= end_parsed
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        if let Some(c) = count {
            result.truncate(c);
        }

        result
    }

    /// XREVRANGE - Get entries in reverse order
    pub fn xrevrange(&self, end: &str, start: &str, count: Option<usize>) -> Vec<StreamEntry> {
        let start_parsed = Self::parse_id(start).unwrap_or((0, 0));
        let end_parsed = Self::parse_id(end).unwrap_or((u64::MAX, u64::MAX));

        let mut result: Vec<StreamEntry> = self
            .entries
            .iter()
            .rev()
            .filter(|entry| {
                if let Some(entry_id) = Self::parse_id(&entry.id) {
                    entry_id >= start_parsed && entry_id <= end_parsed
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        if let Some(c) = count {
            result.truncate(c);
        }

        result
    }

    /// XREAD - Read entries greater than given ID
    pub fn xread(&self, id: &str, count: Option<usize>) -> Vec<StreamEntry> {
        let start_parsed = Self::parse_id(id).unwrap_or((0, 0));

        let mut result: Vec<StreamEntry> = self
            .entries
            .iter()
            .filter(|entry| {
                if let Some(entry_id) = Self::parse_id(&entry.id) {
                    entry_id > start_parsed
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        if let Some(c) = count {
            result.truncate(c);
        }

        result
    }

    /// XTRIM - Trim stream to specified length
    pub fn xtrim(&mut self, max_len: usize, approximate: bool) -> usize {
        let current_len = self.entries.len();
        if current_len <= max_len {
            return 0;
        }

        let to_remove = if approximate {
            // For approximate, we can remove slightly fewer entries
            (current_len - max_len)
                .saturating_sub(100)
                .max(current_len - max_len)
        } else {
            current_len - max_len
        };

        self.entries.drain(0..to_remove);
        to_remove
    }

    /// XDEL - Delete specific entries
    pub fn xdel(&mut self, ids: Vec<String>) -> usize {
        let mut deleted = 0;
        for id in ids {
            if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
                self.entries.remove(pos);
                deleted += 1;
            }
        }
        deleted
    }

    /// XINFO STREAM - Get stream info
    pub fn xinfo_stream(&self) -> HashMap<String, String> {
        let mut info = HashMap::new();
        info.insert("length".to_string(), self.entries.len().to_string());
        info.insert(
            "first-entry-id".to_string(),
            self.entries
                .first()
                .map(|e| e.id.clone())
                .unwrap_or_default(),
        );
        info.insert(
            "last-entry-id".to_string(),
            self.entries
                .last()
                .map(|e| e.id.clone())
                .unwrap_or_default(),
        );
        info.insert("groups".to_string(), self.groups.len().to_string());
        info
    }

    /// XGROUP CREATE - Create a consumer group
    pub fn xgroup_create(&mut self, group_name: &str, start_id: &str) -> Result<(), String> {
        if self.groups.contains_key(group_name) {
            return Err("BUSYGROUP Consumer Group name already exists".to_string());
        }

        let last_delivered = if start_id == "$" {
            self.entries
                .last()
                .map(|e| e.id.clone())
                .unwrap_or_else(|| "0-0".to_string())
        } else if start_id == "0" {
            "0-0".to_string()
        } else {
            start_id.to_string()
        };

        self.groups.insert(
            group_name.to_string(),
            ConsumerGroup {
                name: group_name.to_string(),
                last_delivered_id: last_delivered,
                pending: HashMap::new(),
                consumers: HashMap::new(),
            },
        );

        Ok(())
    }

    /// XGROUP DESTROY - Delete a consumer group
    pub fn xgroup_destroy(&mut self, group_name: &str) -> bool {
        self.groups.remove(group_name).is_some()
    }

    /// XREADGROUP - Read from stream as part of consumer group
    pub fn xreadgroup(
        &mut self,
        group_name: &str,
        consumer_name: &str,
        id: &str,
        count: Option<usize>,
    ) -> Result<Vec<StreamEntry>, String> {
        let group = self
            .groups
            .get_mut(group_name)
            .ok_or_else(|| "NOGROUP No such consumer group".to_string())?;

        // Ensure consumer exists
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        group
            .consumers
            .entry(consumer_name.to_string())
            .or_insert_with(|| StreamConsumer {
                name: consumer_name.to_string(),
                pending_count: 0,
                last_seen: now,
            });

        if id == ">" {
            // Read new messages
            let last_delivered = Self::parse_id(&group.last_delivered_id).unwrap_or((0, 0));

            let mut result: Vec<StreamEntry> = self
                .entries
                .iter()
                .filter(|entry| {
                    if let Some(entry_id) = Self::parse_id(&entry.id) {
                        entry_id > last_delivered
                    } else {
                        false
                    }
                })
                .cloned()
                .collect();

            if let Some(c) = count {
                result.truncate(c);
            }

            // Update last delivered ID and pending
            if let Some(last_entry) = result.last() {
                group.last_delivered_id = last_entry.id.clone();
            }

            // Add to pending
            let pending_list = group.pending.entry(consumer_name.to_string()).or_default();
            for entry in &result {
                pending_list.push(entry.id.clone());
            }

            if let Some(consumer) = group.consumers.get_mut(consumer_name) {
                consumer.pending_count += result.len();
                consumer.last_seen = now;
            }

            Ok(result)
        } else {
            // Read pending messages for this consumer
            let pending = group
                .pending
                .get(consumer_name)
                .cloned()
                .unwrap_or_default();
            let result: Vec<StreamEntry> = self
                .entries
                .iter()
                .filter(|e| pending.contains(&e.id))
                .cloned()
                .collect();
            Ok(result)
        }
    }

    /// XACK - Acknowledge messages
    pub fn xack(&mut self, group_name: &str, ids: Vec<String>) -> Result<usize, String> {
        let group = self
            .groups
            .get_mut(group_name)
            .ok_or_else(|| "NOGROUP No such consumer group".to_string())?;

        let mut acked = 0;
        for (consumer_name, pending_list) in group.pending.iter_mut() {
            let before_len = pending_list.len();
            pending_list.retain(|id| !ids.contains(id));
            let removed = before_len - pending_list.len();
            acked += removed;

            if let Some(consumer) = group.consumers.get_mut(consumer_name) {
                consumer.pending_count = consumer.pending_count.saturating_sub(removed);
            }
        }

        Ok(acked)
    }

    /// XPENDING - Get pending entries info
    pub fn xpending(
        &self,
        group_name: &str,
    ) -> Result<(usize, Option<String>, Option<String>, Vec<(String, usize)>), String> {
        let group = self
            .groups
            .get(group_name)
            .ok_or_else(|| "NOGROUP No such consumer group".to_string())?;

        let mut all_pending: Vec<&String> = group.pending.values().flatten().collect();
        all_pending.sort();

        let min_id = all_pending.first().map(|s| (*s).clone());
        let max_id = all_pending.last().map(|s| (*s).clone());

        let consumers_summary: Vec<(String, usize)> = group
            .pending
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| (k.clone(), v.len()))
            .collect();

        Ok((all_pending.len(), min_id, max_id, consumers_summary))
    }

    /// XSETID - Set the last ID of the stream
    pub fn xsetid(&mut self, id: &str) -> Result<(), String> {
        let parsed = Self::parse_id(id).ok_or_else(|| "ERR Invalid stream ID".to_string())?;
        self.last_id = parsed;
        Ok(())
    }
}

impl Default for StreamActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Addressable for StreamActor {
    fn addressable_type() -> &'static str {
        "StreamActor"
    }
}

impl ActorWithStringKey for StreamActor {}

/// Actor for storing sorted sets (Redis ZSET type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortedSetActor {
    // Map from member to score for fast member lookup
    pub member_scores: std::collections::HashMap<String, f64>,
    // Map from score to set of members with that score (for handling ties)
    pub score_members: std::collections::BTreeMap<
        ordered_float::OrderedFloat<f64>,
        std::collections::HashSet<String>,
    >,
}

impl SortedSetActor {
    pub fn new() -> Self {
        Self {
            member_scores: std::collections::HashMap::new(),
            score_members: std::collections::BTreeMap::new(),
        }
    }

    pub fn zadd(&mut self, member: String, score: f64) -> bool {
        let ordered_score = ordered_float::OrderedFloat(score);

        // Check if member already exists
        if let Some(old_score) = self.member_scores.get(&member) {
            let old_ordered_score = ordered_float::OrderedFloat(*old_score);

            // If the score is the same, no change
            if old_ordered_score == ordered_score {
                return false;
            }

            // Remove member from old score's set
            if let Some(old_set) = self.score_members.get_mut(&old_ordered_score) {
                old_set.remove(&member);
                if old_set.is_empty() {
                    self.score_members.remove(&old_ordered_score);
                }
            }
        }

        // Update member's score
        let was_new = self.member_scores.insert(member.clone(), score).is_none();

        // Add member to new score's set
        self.score_members
            .entry(ordered_score)
            .or_default()
            .insert(member);

        was_new
    }

    pub fn zrem(&mut self, members: Vec<String>) -> usize {
        let mut removed_count = 0;

        for member in members {
            if let Some(score) = self.member_scores.remove(&member) {
                let ordered_score = ordered_float::OrderedFloat(score);

                // Remove member from score's set
                if let Some(score_set) = self.score_members.get_mut(&ordered_score) {
                    score_set.remove(&member);
                    if score_set.is_empty() {
                        self.score_members.remove(&ordered_score);
                    }
                }

                removed_count += 1;
            }
        }

        removed_count
    }

    pub fn zcard(&self) -> usize {
        self.member_scores.len()
    }

    pub fn zscore(&self, member: &str) -> Option<f64> {
        self.member_scores.get(member).copied()
    }

    pub fn zincrby(&mut self, member: String, increment: f64) -> f64 {
        let current_score = self.member_scores.get(&member).copied().unwrap_or(0.0);
        let new_score = current_score + increment;

        // Use zadd to handle the score update properly
        self.zadd(member, new_score);

        new_score
    }

    pub fn zrange(&self, start: i64, stop: i64, with_scores: bool) -> Vec<(String, Option<f64>)> {
        let members: Vec<_> = self
            .score_members
            .iter()
            .flat_map(|(score, members_set)| {
                let score_val = score.0;
                let mut sorted_members: Vec<_> = members_set.iter().cloned().collect();
                sorted_members.sort(); // Sort members lexicographically for consistent ordering
                sorted_members
                    .into_iter()
                    .map(move |member| (member, score_val))
            })
            .collect();

        let len = members.len() as i64;
        if len == 0 {
            return Vec::new();
        }

        // Convert negative indices
        let start_idx = if start < 0 {
            (len + start).max(0)
        } else {
            start.min(len)
        } as usize;

        let stop_idx = if stop < 0 {
            (len + stop + 1).max(0)
        } else {
            (stop + 1).min(len)
        } as usize;

        if start_idx >= members.len() || start_idx >= stop_idx {
            return Vec::new();
        }

        members[start_idx..stop_idx]
            .iter()
            .map(|(member, score)| {
                if with_scores {
                    (member.clone(), Some(*score))
                } else {
                    (member.clone(), None)
                }
            })
            .collect()
    }

    pub fn zrangebyscore(
        &self,
        min_score: f64,
        max_score: f64,
        with_scores: bool,
    ) -> Vec<(String, Option<f64>)> {
        let min_ordered = ordered_float::OrderedFloat(min_score);
        let max_ordered = ordered_float::OrderedFloat(max_score);

        let mut result = Vec::new();

        for (score, members_set) in self.score_members.range(min_ordered..=max_ordered) {
            let score_val = score.0;
            let mut sorted_members: Vec<_> = members_set.iter().cloned().collect();
            sorted_members.sort(); // Sort members lexicographically for consistent ordering

            for member in sorted_members {
                if with_scores {
                    result.push((member, Some(score_val)));
                } else {
                    result.push((member, None));
                }
            }
        }

        result
    }

    pub fn zcount(&self, min_score: f64, max_score: f64) -> usize {
        let min_ordered = ordered_float::OrderedFloat(min_score);
        let max_ordered = ordered_float::OrderedFloat(max_score);

        self.score_members
            .range(min_ordered..=max_ordered)
            .map(|(_, members_set)| members_set.len())
            .sum()
    }

    pub fn zrank(&self, member: &str) -> Option<usize> {
        if let Some(member_score) = self.member_scores.get(member) {
            let member_ordered_score = ordered_float::OrderedFloat(*member_score);
            let mut rank = 0;

            // Count all members with lower scores
            for (score, members_set) in &self.score_members {
                if *score < member_ordered_score {
                    rank += members_set.len();
                } else if *score == member_ordered_score {
                    // Count members with same score that come before this member lexicographically
                    let mut same_score_members: Vec<_> = members_set.iter().collect();
                    same_score_members.sort();

                    for same_member in same_score_members {
                        if same_member == member {
                            return Some(rank);
                        }
                        rank += 1;
                    }
                } else {
                    break;
                }
            }
        }
        None
    }
}

impl Default for SortedSetActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Addressable for SortedSetActor {
    fn addressable_type() -> &'static str {
        "SortedSetActor"
    }
}

impl ActorWithStringKey for SortedSetActor {}

/// Async trait for SortedSetActor methods
#[async_trait]
pub trait SortedSetActorMethods: Addressable {
    async fn zadd(&mut self, member: String, score: f64) -> OrbitResult<bool>;
    async fn zrem(&mut self, members: Vec<String>) -> OrbitResult<usize>;
    async fn zcard(&self) -> OrbitResult<usize>;
    async fn zscore(&self, member: &str) -> OrbitResult<Option<f64>>;
    async fn zincrby(&mut self, member: String, increment: f64) -> OrbitResult<f64>;
    async fn zrange(
        &self,
        start: i64,
        stop: i64,
        with_scores: bool,
    ) -> OrbitResult<Vec<(String, Option<f64>)>>;
    async fn zrangebyscore(
        &self,
        min_score: f64,
        max_score: f64,
        with_scores: bool,
    ) -> OrbitResult<Vec<(String, Option<f64>)>>;
    async fn zcount(&self, min_score: f64, max_score: f64) -> OrbitResult<usize>;
    async fn zrank(&self, member: &str) -> OrbitResult<Option<usize>>;
}

#[async_trait]
impl SortedSetActorMethods for SortedSetActor {
    async fn zadd(&mut self, member: String, score: f64) -> OrbitResult<bool> {
        Ok(self.zadd(member, score))
    }

    async fn zrem(&mut self, members: Vec<String>) -> OrbitResult<usize> {
        Ok(self.zrem(members))
    }

    async fn zcard(&self) -> OrbitResult<usize> {
        Ok(self.zcard())
    }

    async fn zscore(&self, member: &str) -> OrbitResult<Option<f64>> {
        Ok(self.zscore(member))
    }

    async fn zincrby(&mut self, member: String, increment: f64) -> OrbitResult<f64> {
        Ok(self.zincrby(member, increment))
    }

    async fn zrange(
        &self,
        start: i64,
        stop: i64,
        with_scores: bool,
    ) -> OrbitResult<Vec<(String, Option<f64>)>> {
        Ok(self.zrange(start, stop, with_scores))
    }

    async fn zrangebyscore(
        &self,
        min_score: f64,
        max_score: f64,
        with_scores: bool,
    ) -> OrbitResult<Vec<(String, Option<f64>)>> {
        Ok(self.zrangebyscore(min_score, max_score, with_scores))
    }

    async fn zcount(&self, min_score: f64, max_score: f64) -> OrbitResult<usize> {
        Ok(self.zcount(min_score, max_score))
    }

    async fn zrank(&self, member: &str) -> OrbitResult<Option<usize>> {
        Ok(self.zrank(member))
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
        actor.expiration =
            Some((chrono::Utc::now() - chrono::Duration::seconds(1)).timestamp() as u64);
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
    fn test_sorted_set_actor() {
        let mut actor = SortedSetActor::new();
        assert_eq!(actor.zcard(), 0);

        // Add some members with scores
        assert!(actor.zadd("member1".to_string(), 1.0));
        assert!(actor.zadd("member2".to_string(), 2.0));
        assert!(actor.zadd("member3".to_string(), 1.5));
        assert_eq!(actor.zcard(), 3);

        // Test score retrieval
        assert_eq!(actor.zscore("member1"), Some(1.0));
        assert_eq!(actor.zscore("member2"), Some(2.0));
        assert_eq!(actor.zscore("nonexistent"), None);

        // Test zrange (should be sorted by score)
        let range = actor.zrange(0, -1, true);
        assert_eq!(range.len(), 3);
        assert_eq!(range[0], ("member1".to_string(), Some(1.0)));
        assert_eq!(range[1], ("member3".to_string(), Some(1.5)));
        assert_eq!(range[2], ("member2".to_string(), Some(2.0)));

        // Test zrangebyscore
        let score_range = actor.zrangebyscore(1.0, 1.5, false);
        assert_eq!(score_range.len(), 2);
        assert_eq!(score_range[0].0, "member1");
        assert_eq!(score_range[1].0, "member3");

        // Test zcount
        assert_eq!(actor.zcount(1.0, 1.5), 2);
        assert_eq!(actor.zcount(0.0, 0.5), 0);

        // Test zincrby
        let new_score = actor.zincrby("member1".to_string(), 0.5);
        assert_eq!(new_score, 1.5);
        assert_eq!(actor.zscore("member1"), Some(1.5));

        // Test zrem
        let removed = actor.zrem(vec!["member2".to_string(), "nonexistent".to_string()]);
        assert_eq!(removed, 1);
        assert_eq!(actor.zcard(), 2);

        // Test zrank
        assert_eq!(actor.zrank("member1"), Some(0)); // First by score and lex order
        assert_eq!(actor.zrank("member3"), Some(1));
        assert_eq!(actor.zrank("nonexistent"), None);
    }

    #[test]
    fn test_simple_string_creation() {
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
        assert_eq!(
            actor.lrange(0, 100),
            vec!["second", "first", "third", "fourth"]
        ); // Beyond bounds

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
        assert_eq!(actor.subscriber_count(), 10);

        // Test publishing to multiple subscribers
        let delivered_count = actor.publish("broadcast message".to_string());
        assert_eq!(delivered_count, 10);
        assert_eq!(actor.message_count, 1);

        // Test unsubscribe non-existent subscriber
        assert!(!actor.unsubscribe("nonexistent"));
        assert_eq!(actor.subscriber_count(), 10);

        // Test unsubscribe existing subscriber
        assert!(actor.unsubscribe("sub5"));
        assert_eq!(actor.subscriber_count(), 9);

        // Test another message
        let delivered_count2 = actor.publish("another message".to_string());
        assert_eq!(delivered_count2, 9);
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

    #[test]
    fn test_stream_actor_xadd_xlen() {
        let mut actor = StreamActor::new();
        assert_eq!(actor.xlen(), 0);

        // Add entry with auto-generated ID
        let result = actor.xadd(
            Some("*"),
            vec![("field1".to_string(), "value1".to_string())],
        );
        assert!(result.is_ok());
        let id = result.unwrap();
        assert!(id.contains('-'));

        assert_eq!(actor.xlen(), 1);

        // Add another entry
        let result = actor.xadd(
            Some("*"),
            vec![
                ("field2".to_string(), "value2".to_string()),
                ("field3".to_string(), "value3".to_string()),
            ],
        );
        assert!(result.is_ok());
        assert_eq!(actor.xlen(), 2);
    }

    #[test]
    fn test_stream_actor_xadd_explicit_id() {
        let mut actor = StreamActor::new();

        // Add entry with explicit ID
        let result = actor.xadd(
            Some("1000-0"),
            vec![("field1".to_string(), "value1".to_string())],
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "1000-0");

        // Adding with same ID should fail
        let result = actor.xadd(
            Some("1000-0"),
            vec![("field1".to_string(), "value1".to_string())],
        );
        assert!(result.is_err());

        // Adding with lower ID should fail
        let result = actor.xadd(
            Some("500-0"),
            vec![("field1".to_string(), "value1".to_string())],
        );
        assert!(result.is_err());

        // Adding with higher ID should succeed
        let result = actor.xadd(
            Some("2000-0"),
            vec![("field1".to_string(), "value1".to_string())],
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "2000-0");
    }

    #[test]
    fn test_stream_actor_xrange() {
        let mut actor = StreamActor::new();

        // Add entries with explicit IDs
        actor
            .xadd(Some("1000-0"), vec![("a".to_string(), "1".to_string())])
            .unwrap();
        actor
            .xadd(Some("2000-0"), vec![("b".to_string(), "2".to_string())])
            .unwrap();
        actor
            .xadd(Some("3000-0"), vec![("c".to_string(), "3".to_string())])
            .unwrap();

        // Get all entries
        let entries = actor.xrange("-", "+", None);
        assert_eq!(entries.len(), 3);

        // Get range
        let entries = actor.xrange("1500-0", "2500-0", None);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "2000-0");

        // Get with count limit
        let entries = actor.xrange("-", "+", Some(2));
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_stream_actor_xrevrange() {
        let mut actor = StreamActor::new();

        actor
            .xadd(Some("1000-0"), vec![("a".to_string(), "1".to_string())])
            .unwrap();
        actor
            .xadd(Some("2000-0"), vec![("b".to_string(), "2".to_string())])
            .unwrap();
        actor
            .xadd(Some("3000-0"), vec![("c".to_string(), "3".to_string())])
            .unwrap();

        // Get all entries in reverse
        let entries = actor.xrevrange("+", "-", None);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].id, "3000-0");
        assert_eq!(entries[2].id, "1000-0");
    }

    #[test]
    fn test_stream_actor_xread() {
        let mut actor = StreamActor::new();

        actor
            .xadd(Some("1000-0"), vec![("a".to_string(), "1".to_string())])
            .unwrap();
        actor
            .xadd(Some("2000-0"), vec![("b".to_string(), "2".to_string())])
            .unwrap();
        actor
            .xadd(Some("3000-0"), vec![("c".to_string(), "3".to_string())])
            .unwrap();

        // Read entries after 1000-0
        let entries = actor.xread("1000-0", None);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "2000-0");
        assert_eq!(entries[1].id, "3000-0");

        // Read with count
        let entries = actor.xread("0-0", Some(2));
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_stream_actor_xtrim() {
        let mut actor = StreamActor::new();

        for i in 0..10 {
            actor
                .xadd(
                    Some(&format!("{}-0", i * 1000)),
                    vec![("idx".to_string(), i.to_string())],
                )
                .unwrap();
        }
        assert_eq!(actor.xlen(), 10);

        // Trim to 5 entries
        let removed = actor.xtrim(5, false);
        assert_eq!(removed, 5);
        assert_eq!(actor.xlen(), 5);

        // First remaining entry should be 5000-0
        let entries = actor.xrange("-", "+", None);
        assert_eq!(entries[0].id, "5000-0");
    }

    #[test]
    fn test_stream_actor_xdel() {
        let mut actor = StreamActor::new();

        actor
            .xadd(Some("1000-0"), vec![("a".to_string(), "1".to_string())])
            .unwrap();
        actor
            .xadd(Some("2000-0"), vec![("b".to_string(), "2".to_string())])
            .unwrap();
        actor
            .xadd(Some("3000-0"), vec![("c".to_string(), "3".to_string())])
            .unwrap();

        // Delete one entry
        let deleted = actor.xdel(vec!["2000-0".to_string()]);
        assert_eq!(deleted, 1);
        assert_eq!(actor.xlen(), 2);

        // Delete non-existent entry
        let deleted = actor.xdel(vec!["9999-0".to_string()]);
        assert_eq!(deleted, 0);
    }

    #[test]
    fn test_stream_actor_consumer_groups() {
        let mut actor = StreamActor::new();

        // Add some entries
        actor
            .xadd(Some("1000-0"), vec![("a".to_string(), "1".to_string())])
            .unwrap();
        actor
            .xadd(Some("2000-0"), vec![("b".to_string(), "2".to_string())])
            .unwrap();

        // Create consumer group
        let result = actor.xgroup_create("mygroup", "0");
        assert!(result.is_ok());

        // Creating same group again should fail
        let result = actor.xgroup_create("mygroup", "0");
        assert!(result.is_err());

        // Read from group
        let result = actor.xreadgroup("mygroup", "consumer1", ">", None);
        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 2);

        // Check pending
        let (pending_count, min_id, max_id, _consumers) = actor.xpending("mygroup").unwrap();
        assert_eq!(pending_count, 2);
        assert!(min_id.is_some());
        assert!(max_id.is_some());

        // Acknowledge entries
        let acked = actor
            .xack("mygroup", vec!["1000-0".to_string(), "2000-0".to_string()])
            .unwrap();
        assert_eq!(acked, 2);

        // Pending should be empty now
        let (pending_count, _, _, _) = actor.xpending("mygroup").unwrap();
        assert_eq!(pending_count, 0);

        // Destroy group
        let destroyed = actor.xgroup_destroy("mygroup");
        assert!(destroyed);
    }

    #[test]
    fn test_stream_actor_xsetid() {
        let mut actor = StreamActor::new();

        // First add an entry to establish a baseline
        actor
            .xadd(Some("1000-0"), vec![("a".to_string(), "1".to_string())])
            .unwrap();

        // Set ID higher than current
        let result = actor.xsetid("5000-0");
        assert!(result.is_ok());

        // Adding entry with lower ID should fail (now we have entries)
        let result = actor.xadd(Some("4000-0"), vec![("a".to_string(), "1".to_string())]);
        assert!(result.is_err());

        // Adding entry with higher ID should succeed
        let result = actor.xadd(Some("6000-0"), vec![("a".to_string(), "1".to_string())]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stream_actor_xinfo() {
        let mut actor = StreamActor::new();

        actor
            .xadd(Some("1000-0"), vec![("a".to_string(), "1".to_string())])
            .unwrap();
        actor
            .xadd(Some("2000-0"), vec![("b".to_string(), "2".to_string())])
            .unwrap();
        actor.xgroup_create("group1", "0").unwrap();

        let info = actor.xinfo_stream();
        assert_eq!(info.get("length").unwrap(), "2");
        assert_eq!(info.get("first-entry-id").unwrap(), "1000-0");
        assert_eq!(info.get("last-entry-id").unwrap(), "2000-0");
        assert_eq!(info.get("groups").unwrap(), "1");
    }
}
