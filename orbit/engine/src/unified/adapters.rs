//! Protocol adapters for unified cross-protocol storage
//!
//! This module provides adapters that translate between protocol-specific
//! data formats and the universal data model. Each adapter enables a protocol
//! (Redis, PostgreSQL, MySQL, CQL, Cypher, AQL, REST) to read/write data
//! that is accessible to all other protocols.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐   ┌─────────────┐   ┌─────────────┐
//! │   Redis     │   │ PostgreSQL  │   │    CQL      │
//! │   Client    │   │   Client    │   │   Client    │
//! └──────┬──────┘   └──────┬──────┘   └──────┬──────┘
//!        │                 │                 │
//!        v                 v                 v
//! ┌─────────────┐   ┌─────────────┐   ┌─────────────┐
//! │   Redis     │   │ PostgreSQL  │   │    CQL      │
//! │  Adapter    │   │  Adapter    │   │  Adapter    │
//! └──────┬──────┘   └──────┬──────┘   └──────┬──────┘
//!        │                 │                 │
//!        └────────────────┬┴────────────────┘
//!                         │
//!                         v
//!              ┌─────────────────────┐
//!              │  UnifiedStorage     │
//!              └─────────────────────┘
//! ```

use super::operations::{FilterExpression, SortOrder, UniversalOperation};
use super::schema::{Protocol, SchemaRegistry};
use super::storage::{UnifiedStorage, UnifiedStorageError, UnifiedStorageResult};
use super::types::{UniversalResult, UniversalValue};
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

/// Trait for protocol adapters that translate between protocol-specific
/// data formats and the universal data model.
#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    /// Get the protocol this adapter handles
    fn protocol(&self) -> Protocol;

    /// Execute a universal operation and return the result
    async fn execute(&self, operation: UniversalOperation)
        -> UnifiedStorageResult<UniversalResult>;

    /// Translate protocol-specific input to UniversalValue
    fn to_universal(&self, data: &[u8]) -> UnifiedStorageResult<UniversalValue>;

    /// Translate UniversalValue to protocol-specific output
    fn from_universal(&self, value: &UniversalValue) -> UnifiedStorageResult<Vec<u8>>;
}

/// Base adapter implementation with common functionality
pub struct BaseAdapter {
    storage: Arc<UnifiedStorage>,
    schema_registry: Arc<SchemaRegistry>,
    protocol: Protocol,
}

impl BaseAdapter {
    /// Create a new base adapter
    pub fn new(
        storage: Arc<UnifiedStorage>,
        schema_registry: Arc<SchemaRegistry>,
        protocol: Protocol,
    ) -> Self {
        Self {
            storage,
            schema_registry,
            protocol,
        }
    }

    /// Get the underlying storage
    pub fn storage(&self) -> &Arc<UnifiedStorage> {
        &self.storage
    }

    /// Get the schema registry
    pub fn schema_registry(&self) -> &Arc<SchemaRegistry> {
        &self.schema_registry
    }

    /// Get the protocol
    pub fn protocol(&self) -> Protocol {
        self.protocol
    }
}

// =============================================================================
// Redis Adapter
// =============================================================================

/// Redis protocol adapter
///
/// Translates Redis RESP commands to UniversalOperations and back.
/// Redis keys are mapped to namespace:key format where the namespace
/// defaults to "redis" unless specified with a colon separator.
pub struct RedisAdapter {
    base: BaseAdapter,
    default_namespace: String,
}

impl RedisAdapter {
    /// Create a new Redis adapter
    pub fn new(storage: Arc<UnifiedStorage>, schema_registry: Arc<SchemaRegistry>) -> Self {
        Self {
            base: BaseAdapter::new(storage, schema_registry, Protocol::Redis),
            default_namespace: "redis".to_string(),
        }
    }

    /// Create with a custom default namespace
    pub fn with_namespace(
        storage: Arc<UnifiedStorage>,
        schema_registry: Arc<SchemaRegistry>,
        namespace: String,
    ) -> Self {
        Self {
            base: BaseAdapter::new(storage, schema_registry, Protocol::Redis),
            default_namespace: namespace,
        }
    }

    /// Parse a Redis key into namespace and key components
    pub fn parse_key(&self, redis_key: &str) -> (String, String) {
        if let Some((namespace, key)) = redis_key.split_once(':') {
            (namespace.to_string(), key.to_string())
        } else {
            (self.default_namespace.clone(), redis_key.to_string())
        }
    }

    /// Format namespace and key as a Redis key
    pub fn format_key(&self, namespace: &str, key: &str) -> String {
        if namespace == self.default_namespace {
            key.to_string()
        } else {
            format!("{}:{}", namespace, key)
        }
    }

    // Redis string commands
    pub async fn get(&self, key: &str) -> UnifiedStorageResult<Option<UniversalValue>> {
        let (namespace, key) = self.parse_key(key);
        let result = self.base.storage.get(&namespace, &key).await?;
        match result {
            UniversalResult::Record(record) => Ok(Some(record.value)),
            UniversalResult::Empty => Ok(None),
            _ => Ok(None),
        }
    }

    pub async fn set(&self, key: &str, value: UniversalValue) -> UnifiedStorageResult<()> {
        let (namespace, key) = self.parse_key(key);
        self.base
            .storage
            .put(&namespace, &key, value, None, false, None)
            .await?;
        Ok(())
    }

    pub async fn set_ex(
        &self,
        key: &str,
        value: UniversalValue,
        seconds: u64,
    ) -> UnifiedStorageResult<()> {
        let (namespace, key) = self.parse_key(key);
        let ttl = Some(Duration::from_secs(seconds));
        self.base
            .storage
            .put(&namespace, &key, value, ttl, false, None)
            .await?;
        Ok(())
    }

    pub async fn setnx(&self, key: &str, value: UniversalValue) -> UnifiedStorageResult<bool> {
        let (namespace, key) = self.parse_key(key);
        match self
            .base
            .storage
            .put(&namespace, &key, value, None, true, None)
            .await
        {
            Ok(_) => Ok(true),
            Err(UnifiedStorageError::KeyExists { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn del(&self, keys: &[&str]) -> UnifiedStorageResult<u64> {
        let mut count = 0u64;
        for key in keys {
            let (namespace, key) = self.parse_key(key);
            let result = self.base.storage.delete(&namespace, &key).await?;
            if let UniversalResult::Count(c) = result {
                count += c;
            }
        }
        Ok(count)
    }

    pub async fn exists(&self, keys: &[&str]) -> UnifiedStorageResult<u64> {
        let mut count = 0u64;
        for key in keys {
            let (namespace, key) = self.parse_key(key);
            let result = self.base.storage.exists(&namespace, &key).await?;
            if let UniversalResult::Value(UniversalValue::Bool(true)) = result {
                count += 1;
            }
        }
        Ok(count)
    }

    pub async fn incr(&self, key: &str) -> UnifiedStorageResult<i64> {
        self.incrby(key, 1).await
    }

    pub async fn incrby(&self, key: &str, delta: i64) -> UnifiedStorageResult<i64> {
        let (namespace, key) = self.parse_key(key);

        // Get current value
        let current = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => match record.value {
                UniversalValue::Int(i) => i,
                UniversalValue::String(s) => s.parse::<i64>().unwrap_or(0),
                _ => 0,
            },
            _ => 0,
        };

        let new_value = current + delta;
        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::Int(new_value),
                None,
                false,
                None,
            )
            .await?;
        Ok(new_value)
    }

    // Redis hash commands
    pub async fn hget(
        &self,
        key: &str,
        field: &str,
    ) -> UnifiedStorageResult<Option<UniversalValue>> {
        let (namespace, key) = self.parse_key(key);
        let result = self.base.storage.get(&namespace, &key).await?;

        match result {
            UniversalResult::Record(record) => {
                if let UniversalValue::Map(map) = record.value {
                    Ok(map.get(field).cloned())
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    pub async fn hset(
        &self,
        key: &str,
        field: &str,
        value: UniversalValue,
    ) -> UnifiedStorageResult<bool> {
        let (namespace, key) = self.parse_key(key);

        // Get current record or create new one
        let mut map = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::Map(m) = record.value {
                    m
                } else {
                    BTreeMap::new()
                }
            }
            _ => BTreeMap::new(),
        };

        let is_new = !map.contains_key(field);
        map.insert(field.to_string(), value);

        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::Map(map),
                None,
                false,
                None,
            )
            .await?;
        Ok(is_new)
    }

    pub async fn hmset(
        &self,
        key: &str,
        fields: Vec<(String, UniversalValue)>,
    ) -> UnifiedStorageResult<()> {
        let (namespace, key) = self.parse_key(key);

        let mut map = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::Map(m) = record.value {
                    m
                } else {
                    BTreeMap::new()
                }
            }
            _ => BTreeMap::new(),
        };

        for (field, value) in fields {
            map.insert(field, value);
        }

        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::Map(map),
                None,
                false,
                None,
            )
            .await?;
        Ok(())
    }

    pub async fn hgetall(
        &self,
        key: &str,
    ) -> UnifiedStorageResult<Option<BTreeMap<String, UniversalValue>>> {
        let (namespace, key) = self.parse_key(key);
        let result = self.base.storage.get(&namespace, &key).await?;

        match result {
            UniversalResult::Record(record) => {
                if let UniversalValue::Map(map) = record.value {
                    Ok(Some(map))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    pub async fn hdel(&self, key: &str, fields: &[&str]) -> UnifiedStorageResult<u64> {
        let (namespace, key) = self.parse_key(key);

        let mut map = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::Map(m) = record.value {
                    m
                } else {
                    return Ok(0);
                }
            }
            _ => return Ok(0),
        };

        let mut count = 0u64;
        for field in fields {
            if map.remove(*field).is_some() {
                count += 1;
            }
        }

        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::Map(map),
                None,
                false,
                None,
            )
            .await?;
        Ok(count)
    }

    pub async fn hincrby(&self, key: &str, field: &str, delta: i64) -> UnifiedStorageResult<i64> {
        let (namespace, key) = self.parse_key(key);

        let mut map = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::Map(m) = record.value {
                    m
                } else {
                    BTreeMap::new()
                }
            }
            _ => BTreeMap::new(),
        };

        let current = map.get(field).and_then(|v| v.as_int()).unwrap_or(0);

        let new_value = current + delta;
        map.insert(field.to_string(), UniversalValue::Int(new_value));

        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::Map(map),
                None,
                false,
                None,
            )
            .await?;
        Ok(new_value)
    }

    // Redis list commands
    pub async fn lpush(
        &self,
        key: &str,
        values: Vec<UniversalValue>,
    ) -> UnifiedStorageResult<usize> {
        let (namespace, key) = self.parse_key(key);

        let mut list = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::List(l) = record.value {
                    l
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };

        // Insert at front (in reverse order to maintain expected behavior)
        for value in values.into_iter().rev() {
            list.insert(0, value);
        }

        let len = list.len();
        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::List(list),
                None,
                false,
                None,
            )
            .await?;
        Ok(len)
    }

    pub async fn rpush(
        &self,
        key: &str,
        values: Vec<UniversalValue>,
    ) -> UnifiedStorageResult<usize> {
        let (namespace, key) = self.parse_key(key);

        let mut list = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::List(l) = record.value {
                    l
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };

        list.extend(values);

        let len = list.len();
        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::List(list),
                None,
                false,
                None,
            )
            .await?;
        Ok(len)
    }

    pub async fn lpop(
        &self,
        key: &str,
        count: Option<usize>,
    ) -> UnifiedStorageResult<Vec<UniversalValue>> {
        let (namespace, key) = self.parse_key(key);

        let mut list = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::List(l) = record.value {
                    l
                } else {
                    return Ok(Vec::new());
                }
            }
            _ => return Ok(Vec::new()),
        };

        let count = count.unwrap_or(1);
        let mut popped = Vec::new();

        for _ in 0..count {
            if list.is_empty() {
                break;
            }
            popped.push(list.remove(0));
        }

        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::List(list),
                None,
                false,
                None,
            )
            .await?;
        Ok(popped)
    }

    pub async fn rpop(
        &self,
        key: &str,
        count: Option<usize>,
    ) -> UnifiedStorageResult<Vec<UniversalValue>> {
        let (namespace, key) = self.parse_key(key);

        let mut list = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::List(l) = record.value {
                    l
                } else {
                    return Ok(Vec::new());
                }
            }
            _ => return Ok(Vec::new()),
        };

        let count = count.unwrap_or(1);
        let mut popped = Vec::new();

        for _ in 0..count {
            if list.is_empty() {
                break;
            }
            popped.push(list.pop().unwrap());
        }

        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::List(list),
                None,
                false,
                None,
            )
            .await?;
        Ok(popped)
    }

    pub async fn lrange(
        &self,
        key: &str,
        start: i64,
        stop: i64,
    ) -> UnifiedStorageResult<Vec<UniversalValue>> {
        let (namespace, key) = self.parse_key(key);

        let list = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::List(l) = record.value {
                    l
                } else {
                    return Ok(Vec::new());
                }
            }
            _ => return Ok(Vec::new()),
        };

        let len = list.len() as i64;
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

        if start >= stop {
            return Ok(Vec::new());
        }

        Ok(list[start..stop].to_vec())
    }

    pub async fn llen(&self, key: &str) -> UnifiedStorageResult<usize> {
        let (namespace, key) = self.parse_key(key);

        let list = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::List(l) = record.value {
                    l
                } else {
                    return Ok(0);
                }
            }
            _ => return Ok(0),
        };

        Ok(list.len())
    }

    // Redis set commands
    pub async fn sadd(
        &self,
        key: &str,
        members: Vec<UniversalValue>,
    ) -> UnifiedStorageResult<usize> {
        let (namespace, key) = self.parse_key(key);

        let mut set = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::Set(s) = record.value {
                    s
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };

        let mut added = 0usize;
        for member in members {
            if !set.contains(&member) {
                set.push(member);
                added += 1;
            }
        }

        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::Set(set),
                None,
                false,
                None,
            )
            .await?;
        Ok(added)
    }

    pub async fn srem(
        &self,
        key: &str,
        members: Vec<UniversalValue>,
    ) -> UnifiedStorageResult<usize> {
        let (namespace, key) = self.parse_key(key);

        let mut set = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::Set(s) = record.value {
                    s
                } else {
                    return Ok(0);
                }
            }
            _ => return Ok(0),
        };

        let mut removed = 0usize;
        for member in members {
            if let Some(pos) = set.iter().position(|x| x == &member) {
                set.remove(pos);
                removed += 1;
            }
        }

        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::Set(set),
                None,
                false,
                None,
            )
            .await?;
        Ok(removed)
    }

    pub async fn sismember(
        &self,
        key: &str,
        member: &UniversalValue,
    ) -> UnifiedStorageResult<bool> {
        let (namespace, key) = self.parse_key(key);

        let set = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::Set(s) = record.value {
                    s
                } else {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        };

        Ok(set.contains(member))
    }

    pub async fn smembers(&self, key: &str) -> UnifiedStorageResult<Vec<UniversalValue>> {
        let (namespace, key) = self.parse_key(key);

        let set = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::Set(s) = record.value {
                    s
                } else {
                    return Ok(Vec::new());
                }
            }
            _ => return Ok(Vec::new()),
        };

        Ok(set)
    }

    // Redis sorted set commands
    pub async fn zadd(
        &self,
        key: &str,
        members: Vec<(UniversalValue, f64)>,
    ) -> UnifiedStorageResult<usize> {
        let (namespace, key) = self.parse_key(key);

        let mut sorted_set = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::SortedSet(ss) = record.value {
                    ss
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };

        let mut added = 0usize;
        for (member, score) in members {
            if let Some(pos) = sorted_set.iter().position(|(m, _)| m == &member) {
                sorted_set[pos].1 = score; // Update score if exists
            } else {
                sorted_set.push((member, score));
                added += 1;
            }
        }

        // Sort by score
        sorted_set.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::SortedSet(sorted_set),
                None,
                false,
                None,
            )
            .await?;
        Ok(added)
    }

    pub async fn zrangebyscore(
        &self,
        key: &str,
        min: f64,
        max: f64,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> UnifiedStorageResult<Vec<(UniversalValue, f64)>> {
        let (namespace, key) = self.parse_key(key);

        let sorted_set = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::SortedSet(ss) = record.value {
                    ss
                } else {
                    return Ok(Vec::new());
                }
            }
            _ => return Ok(Vec::new()),
        };

        let filtered: Vec<_> = sorted_set
            .into_iter()
            .filter(|(_, score)| *score >= min && *score <= max)
            .collect();

        let offset = offset.unwrap_or(0);
        let result: Vec<_> = filtered
            .into_iter()
            .skip(offset)
            .take(limit.unwrap_or(usize::MAX))
            .collect();

        Ok(result)
    }

    pub async fn zrange(
        &self,
        key: &str,
        start: i64,
        stop: i64,
        with_scores: bool,
    ) -> UnifiedStorageResult<Vec<(UniversalValue, f64)>> {
        let (namespace, key) = self.parse_key(key);

        let sorted_set = match self.base.storage.get(&namespace, &key).await? {
            UniversalResult::Record(record) => {
                if let UniversalValue::SortedSet(ss) = record.value {
                    ss
                } else {
                    return Ok(Vec::new());
                }
            }
            _ => return Ok(Vec::new()),
        };

        let len = sorted_set.len() as i64;
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

        if start >= stop {
            return Ok(Vec::new());
        }

        let result: Vec<_> = sorted_set[start..stop]
            .iter()
            .map(|(m, s)| (m.clone(), if with_scores { *s } else { 0.0 }))
            .collect();

        Ok(result)
    }

    // TTL commands
    pub async fn expire(&self, key: &str, seconds: u64) -> UnifiedStorageResult<bool> {
        let (namespace, key) = self.parse_key(key);

        // Check if key exists
        let result = self.base.storage.get(&namespace, &key).await?;
        match result {
            UniversalResult::Record(record) => {
                // Re-put with TTL
                self.base
                    .storage
                    .put(
                        &namespace,
                        &key,
                        record.value,
                        Some(Duration::from_secs(seconds)),
                        false,
                        None,
                    )
                    .await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub async fn ttl(&self, _key: &str) -> UnifiedStorageResult<i64> {
        // TTL tracking would require additional metadata storage
        // For now, return -1 (no TTL) or -2 (key doesn't exist)
        Ok(-1)
    }

    // Key scanning
    pub async fn keys(&self, pattern: &str) -> UnifiedStorageResult<Vec<String>> {
        let result = self
            .base
            .storage
            .scan(&self.default_namespace, None, None, None, None, Vec::new())
            .await?;

        if let UniversalResult::Records(records) = result {
            let keys: Vec<String> = records
                .into_iter()
                .map(|r| self.format_key(&r.id.namespace, &r.id.key))
                .filter(|k| Self::matches_pattern(k, pattern))
                .collect();
            Ok(keys)
        } else {
            Ok(Vec::new())
        }
    }

    fn matches_pattern(key: &str, pattern: &str) -> bool {
        // Simple glob pattern matching
        if pattern == "*" {
            return true;
        }

        let regex_pattern = pattern.replace("*", ".*").replace("?", ".");

        regex::Regex::new(&format!("^{}$", regex_pattern))
            .map(|re| re.is_match(key))
            .unwrap_or(false)
    }
}

#[async_trait]
impl ProtocolAdapter for RedisAdapter {
    fn protocol(&self) -> Protocol {
        Protocol::Redis
    }

    async fn execute(
        &self,
        operation: UniversalOperation,
    ) -> UnifiedStorageResult<UniversalResult> {
        // Delegate to storage layer
        match operation {
            UniversalOperation::Get { namespace, key } => {
                self.base.storage.get(&namespace, &key).await
            }
            UniversalOperation::Put {
                namespace,
                key,
                value,
                ttl,
                if_not_exists,
                if_version,
            } => {
                self.base
                    .storage
                    .put(&namespace, &key, value, ttl, if_not_exists, if_version)
                    .await
            }
            UniversalOperation::Delete { namespace, key } => {
                self.base.storage.delete(&namespace, &key).await
            }
            UniversalOperation::Exists { namespace, key } => {
                self.base.storage.exists(&namespace, &key).await
            }
            _ => Err(UnifiedStorageError::NotImplemented(
                "Operation not supported".to_string(),
            )),
        }
    }

    fn to_universal(&self, data: &[u8]) -> UnifiedStorageResult<UniversalValue> {
        let s = String::from_utf8(data.to_vec())
            .map_err(|e| UnifiedStorageError::SerializationError(e.to_string()))?;
        Ok(UniversalValue::String(s))
    }

    fn from_universal(&self, value: &UniversalValue) -> UnifiedStorageResult<Vec<u8>> {
        match value {
            UniversalValue::String(s) => Ok(s.as_bytes().to_vec()),
            UniversalValue::Int(i) => Ok(i.to_string().into_bytes()),
            UniversalValue::Float(f) => Ok(f.to_string().into_bytes()),
            UniversalValue::Bool(b) => Ok(if *b { b"1" } else { b"0" }.to_vec()),
            UniversalValue::Null => Ok(b"".to_vec()),
            other => {
                let json = serde_json::to_string(other)
                    .map_err(|e| UnifiedStorageError::SerializationError(e.to_string()))?;
                Ok(json.into_bytes())
            }
        }
    }
}

// =============================================================================
// SQL Adapter (shared by PostgreSQL and MySQL)
// =============================================================================

/// SQL protocol adapter for PostgreSQL and MySQL
pub struct SqlAdapter {
    base: BaseAdapter,
}

impl SqlAdapter {
    /// Create a new SQL adapter for PostgreSQL
    pub fn postgres(storage: Arc<UnifiedStorage>, schema_registry: Arc<SchemaRegistry>) -> Self {
        Self {
            base: BaseAdapter::new(storage, schema_registry, Protocol::PostgreSQL),
        }
    }

    /// Create a new SQL adapter for MySQL
    pub fn mysql(storage: Arc<UnifiedStorage>, schema_registry: Arc<SchemaRegistry>) -> Self {
        Self {
            base: BaseAdapter::new(storage, schema_registry, Protocol::MySQL),
        }
    }

    /// Insert a row into a table
    pub async fn insert(
        &self,
        table: &str,
        row: BTreeMap<String, UniversalValue>,
        primary_key: &str,
    ) -> UnifiedStorageResult<()> {
        let key = row
            .get(primary_key)
            .and_then(|v| match v {
                UniversalValue::String(s) => Some(s.clone()),
                UniversalValue::Int(i) => Some(i.to_string()),
                _ => None,
            })
            .ok_or_else(|| {
                UnifiedStorageError::InvalidData(format!("Missing primary key: {}", primary_key))
            })?;

        self.base
            .storage
            .put(table, &key, UniversalValue::Map(row), None, false, None)
            .await?;
        Ok(())
    }

    /// Select rows from a table
    pub async fn select(
        &self,
        table: &str,
        columns: Option<Vec<String>>,
        filter: Option<FilterExpression>,
        order_by: Option<Vec<(String, SortOrder)>>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> UnifiedStorageResult<Vec<BTreeMap<String, UniversalValue>>> {
        let projection = columns.unwrap_or_default();
        let result = self
            .base
            .storage
            .scan(table, filter, limit, offset, order_by, projection)
            .await?;

        let rows: Vec<BTreeMap<String, UniversalValue>> = match result {
            UniversalResult::Records(records) => records
                .into_iter()
                .filter_map(|r| {
                    if let UniversalValue::Map(map) = r.value {
                        Some(map)
                    } else {
                        None
                    }
                })
                .collect(),
            _ => Vec::new(),
        };

        Ok(rows)
    }

    /// Update rows in a table
    pub async fn update(
        &self,
        table: &str,
        updates: BTreeMap<String, UniversalValue>,
        filter: Option<FilterExpression>,
    ) -> UnifiedStorageResult<u64> {
        let result = self
            .base
            .storage
            .scan(table, filter, None, None, None, Vec::new())
            .await?;
        let mut count = 0u64;

        if let UniversalResult::Records(records) = result {
            for record in records {
                if let UniversalValue::Map(mut map) = record.value {
                    for (field, value) in &updates {
                        map.insert(field.clone(), value.clone());
                    }
                    self.base
                        .storage
                        .put(
                            table,
                            &record.id.key,
                            UniversalValue::Map(map),
                            None,
                            false,
                            None,
                        )
                        .await?;
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Delete rows from a table
    pub async fn delete(
        &self,
        table: &str,
        filter: Option<FilterExpression>,
    ) -> UnifiedStorageResult<u64> {
        let result = self
            .base
            .storage
            .scan(table, filter, None, None, None, Vec::new())
            .await?;

        if let UniversalResult::Records(records) = result {
            let keys: Vec<String> = records.into_iter().map(|r| r.id.key).collect();
            let result = self.base.storage.multi_delete(table, &keys).await?;
            if let UniversalResult::Count(c) = result {
                return Ok(c);
            }
        }
        Ok(0)
    }

    /// Count rows in a table
    pub async fn count(
        &self,
        table: &str,
        filter: Option<FilterExpression>,
    ) -> UnifiedStorageResult<u64> {
        let result = self
            .base
            .storage
            .scan(table, filter, None, None, None, Vec::new())
            .await?;

        if let UniversalResult::Records(records) = result {
            Ok(records.len() as u64)
        } else {
            Ok(0)
        }
    }
}

#[async_trait]
impl ProtocolAdapter for SqlAdapter {
    fn protocol(&self) -> Protocol {
        self.base.protocol
    }

    async fn execute(
        &self,
        operation: UniversalOperation,
    ) -> UnifiedStorageResult<UniversalResult> {
        match operation {
            UniversalOperation::Get { namespace, key } => {
                self.base.storage.get(&namespace, &key).await
            }
            UniversalOperation::Put {
                namespace,
                key,
                value,
                ttl,
                if_not_exists,
                if_version,
            } => {
                self.base
                    .storage
                    .put(&namespace, &key, value, ttl, if_not_exists, if_version)
                    .await
            }
            UniversalOperation::Delete { namespace, key } => {
                self.base.storage.delete(&namespace, &key).await
            }
            UniversalOperation::Scan {
                namespace,
                filter,
                limit,
                offset,
                order_by,
                projection,
            } => {
                self.base
                    .storage
                    .scan(&namespace, filter, limit, offset, order_by, projection)
                    .await
            }
            _ => Err(UnifiedStorageError::NotImplemented(
                "Operation not supported".to_string(),
            )),
        }
    }

    fn to_universal(&self, data: &[u8]) -> UnifiedStorageResult<UniversalValue> {
        serde_json::from_slice(data)
            .map_err(|e| UnifiedStorageError::SerializationError(e.to_string()))
    }

    fn from_universal(&self, value: &UniversalValue) -> UnifiedStorageResult<Vec<u8>> {
        serde_json::to_vec(value)
            .map_err(|e| UnifiedStorageError::SerializationError(e.to_string()))
    }
}

// =============================================================================
// CQL Adapter
// =============================================================================

/// CQL (Cassandra Query Language) protocol adapter
pub struct CqlAdapter {
    base: BaseAdapter,
    current_keyspace: Option<String>,
}

impl CqlAdapter {
    /// Create a new CQL adapter
    pub fn new(storage: Arc<UnifiedStorage>, schema_registry: Arc<SchemaRegistry>) -> Self {
        Self {
            base: BaseAdapter::new(storage, schema_registry, Protocol::CQL),
            current_keyspace: None,
        }
    }

    /// Set the current keyspace
    pub fn use_keyspace(&mut self, keyspace: &str) {
        self.current_keyspace = Some(keyspace.to_string());
    }

    /// Get the full namespace for a table
    fn full_namespace(&self, table: &str) -> String {
        if let Some(ref ks) = self.current_keyspace {
            format!("{}:{}", ks, table)
        } else {
            table.to_string()
        }
    }

    /// Insert a row
    pub async fn insert(
        &self,
        table: &str,
        row: BTreeMap<String, UniversalValue>,
        partition_key: &str,
    ) -> UnifiedStorageResult<()> {
        let namespace = self.full_namespace(table);
        let key = row
            .get(partition_key)
            .and_then(|v| match v {
                UniversalValue::String(s) => Some(s.clone()),
                UniversalValue::Int(i) => Some(i.to_string()),
                UniversalValue::Uuid(u) => Some(uuid::Uuid::from_bytes(*u).to_string()),
                _ => None,
            })
            .ok_or_else(|| {
                UnifiedStorageError::InvalidData(format!(
                    "Missing partition key: {}",
                    partition_key
                ))
            })?;

        self.base
            .storage
            .put(
                &namespace,
                &key,
                UniversalValue::Map(row),
                None,
                false,
                None,
            )
            .await?;
        Ok(())
    }

    /// Select rows
    pub async fn select(
        &self,
        table: &str,
        columns: Option<Vec<String>>,
        filter: Option<FilterExpression>,
        limit: Option<usize>,
    ) -> UnifiedStorageResult<Vec<BTreeMap<String, UniversalValue>>> {
        let namespace = self.full_namespace(table);
        let projection = columns.unwrap_or_default();
        let result = self
            .base
            .storage
            .scan(&namespace, filter, limit, None, None, projection)
            .await?;

        let rows: Vec<BTreeMap<String, UniversalValue>> = match result {
            UniversalResult::Records(records) => records
                .into_iter()
                .filter_map(|r| {
                    if let UniversalValue::Map(map) = r.value {
                        Some(map)
                    } else {
                        None
                    }
                })
                .collect(),
            _ => Vec::new(),
        };

        Ok(rows)
    }
}

#[async_trait]
impl ProtocolAdapter for CqlAdapter {
    fn protocol(&self) -> Protocol {
        Protocol::CQL
    }

    async fn execute(
        &self,
        operation: UniversalOperation,
    ) -> UnifiedStorageResult<UniversalResult> {
        match operation {
            UniversalOperation::Get { namespace, key } => {
                self.base.storage.get(&namespace, &key).await
            }
            UniversalOperation::Put {
                namespace,
                key,
                value,
                ttl,
                if_not_exists,
                if_version,
            } => {
                self.base
                    .storage
                    .put(&namespace, &key, value, ttl, if_not_exists, if_version)
                    .await
            }
            _ => Err(UnifiedStorageError::NotImplemented(
                "Operation not supported".to_string(),
            )),
        }
    }

    fn to_universal(&self, data: &[u8]) -> UnifiedStorageResult<UniversalValue> {
        serde_json::from_slice(data)
            .map_err(|e| UnifiedStorageError::SerializationError(e.to_string()))
    }

    fn from_universal(&self, value: &UniversalValue) -> UnifiedStorageResult<Vec<u8>> {
        serde_json::to_vec(value)
            .map_err(|e| UnifiedStorageError::SerializationError(e.to_string()))
    }
}

// =============================================================================
// Graph Adapter (Cypher/AQL)
// =============================================================================

/// Graph protocol adapter for Cypher (Neo4j) and AQL (ArangoDB)
pub struct GraphAdapter {
    base: BaseAdapter,
    node_namespace: String,
    edge_namespace: String,
}

impl GraphAdapter {
    /// Create a new Cypher adapter
    pub fn cypher(storage: Arc<UnifiedStorage>, schema_registry: Arc<SchemaRegistry>) -> Self {
        Self {
            base: BaseAdapter::new(storage, schema_registry, Protocol::Cypher),
            node_namespace: "graph:nodes".to_string(),
            edge_namespace: "graph:edges".to_string(),
        }
    }

    /// Create a new AQL adapter
    pub fn aql(storage: Arc<UnifiedStorage>, schema_registry: Arc<SchemaRegistry>) -> Self {
        Self {
            base: BaseAdapter::new(storage, schema_registry, Protocol::AQL),
            node_namespace: "graph:nodes".to_string(),
            edge_namespace: "graph:edges".to_string(),
        }
    }

    /// Create a node
    pub async fn create_node(
        &self,
        id: &str,
        labels: Vec<String>,
        properties: BTreeMap<String, UniversalValue>,
    ) -> UnifiedStorageResult<()> {
        let node = UniversalValue::Node {
            id: id.to_string(),
            labels,
            properties,
        };

        self.base
            .storage
            .put(&self.node_namespace, id, node, None, false, None)
            .await?;
        Ok(())
    }

    /// Create a relationship
    pub async fn create_relationship(
        &self,
        id: &str,
        rel_type: &str,
        start_node: &str,
        end_node: &str,
        properties: BTreeMap<String, UniversalValue>,
    ) -> UnifiedStorageResult<()> {
        let edge = UniversalValue::Relationship {
            id: id.to_string(),
            rel_type: rel_type.to_string(),
            start_node: start_node.to_string(),
            end_node: end_node.to_string(),
            properties,
        };

        self.base
            .storage
            .put(&self.edge_namespace, id, edge, None, false, None)
            .await?;
        Ok(())
    }

    /// Get a node by ID
    pub async fn get_node(&self, id: &str) -> UnifiedStorageResult<Option<UniversalValue>> {
        let result = self.base.storage.get(&self.node_namespace, id).await?;
        match result {
            UniversalResult::Record(record) => Ok(Some(record.value)),
            _ => Ok(None),
        }
    }

    /// Get nodes by label
    pub async fn get_nodes_by_label(
        &self,
        label: &str,
    ) -> UnifiedStorageResult<Vec<UniversalValue>> {
        let result = self
            .base
            .storage
            .scan(&self.node_namespace, None, None, None, None, Vec::new())
            .await?;

        let nodes: Vec<UniversalValue> = match result {
            UniversalResult::Records(records) => records
                .into_iter()
                .filter_map(|r| {
                    if let UniversalValue::Node { ref labels, .. } = r.value {
                        if labels.contains(&label.to_string()) {
                            return Some(r.value);
                        }
                    }
                    None
                })
                .collect(),
            _ => Vec::new(),
        };

        Ok(nodes)
    }

    /// Get relationships for a node
    pub async fn get_relationships(
        &self,
        node_id: &str,
        direction: Option<&str>,
        rel_type: Option<&str>,
    ) -> UnifiedStorageResult<Vec<UniversalValue>> {
        let result = self
            .base
            .storage
            .scan(&self.edge_namespace, None, None, None, None, Vec::new())
            .await?;

        let edges: Vec<UniversalValue> = match result {
            UniversalResult::Records(records) => records
                .into_iter()
                .filter_map(|r| {
                    if let UniversalValue::Relationship {
                        ref start_node,
                        ref end_node,
                        rel_type: ref rt,
                        ..
                    } = r.value
                    {
                        if let Some(filter_type) = rel_type {
                            if rt != filter_type {
                                return None;
                            }
                        }

                        match direction {
                            Some("outgoing") => {
                                if start_node == node_id {
                                    return Some(r.value);
                                }
                            }
                            Some("incoming") => {
                                if end_node == node_id {
                                    return Some(r.value);
                                }
                            }
                            _ => {
                                if start_node == node_id || end_node == node_id {
                                    return Some(r.value);
                                }
                            }
                        }
                    }
                    None
                })
                .collect(),
            _ => Vec::new(),
        };

        Ok(edges)
    }
}

#[async_trait]
impl ProtocolAdapter for GraphAdapter {
    fn protocol(&self) -> Protocol {
        self.base.protocol
    }

    async fn execute(
        &self,
        operation: UniversalOperation,
    ) -> UnifiedStorageResult<UniversalResult> {
        match operation {
            UniversalOperation::Get { namespace, key } => {
                self.base.storage.get(&namespace, &key).await
            }
            _ => Err(UnifiedStorageError::NotImplemented(
                "Operation not supported".to_string(),
            )),
        }
    }

    fn to_universal(&self, data: &[u8]) -> UnifiedStorageResult<UniversalValue> {
        serde_json::from_slice(data)
            .map_err(|e| UnifiedStorageError::SerializationError(e.to_string()))
    }

    fn from_universal(&self, value: &UniversalValue) -> UnifiedStorageResult<Vec<u8>> {
        serde_json::to_vec(value)
            .map_err(|e| UnifiedStorageError::SerializationError(e.to_string()))
    }
}

// =============================================================================
// REST Adapter
// =============================================================================

/// REST/HTTP protocol adapter
pub struct RestAdapter {
    base: BaseAdapter,
}

impl RestAdapter {
    /// Create a new REST adapter
    pub fn new(storage: Arc<UnifiedStorage>, schema_registry: Arc<SchemaRegistry>) -> Self {
        Self {
            base: BaseAdapter::new(storage, schema_registry, Protocol::REST),
        }
    }

    /// GET /resource/:id
    pub async fn get(
        &self,
        resource: &str,
        id: &str,
    ) -> UnifiedStorageResult<Option<UniversalValue>> {
        let result = self.base.storage.get(resource, id).await?;
        match result {
            UniversalResult::Record(record) => Ok(Some(record.value)),
            _ => Ok(None),
        }
    }

    /// GET /resource (list all)
    pub async fn list(
        &self,
        resource: &str,
        filter: Option<FilterExpression>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> UnifiedStorageResult<Vec<UniversalValue>> {
        let result = self
            .base
            .storage
            .scan(resource, filter, limit, offset, None, Vec::new())
            .await?;
        match result {
            UniversalResult::Records(records) => Ok(records.into_iter().map(|r| r.value).collect()),
            _ => Ok(Vec::new()),
        }
    }

    /// POST /resource (create)
    pub async fn create(
        &self,
        resource: &str,
        id: &str,
        data: UniversalValue,
    ) -> UnifiedStorageResult<()> {
        self.base
            .storage
            .put(resource, id, data, None, false, None)
            .await?;
        Ok(())
    }

    /// PUT /resource/:id (update)
    pub async fn update(
        &self,
        resource: &str,
        id: &str,
        data: UniversalValue,
    ) -> UnifiedStorageResult<()> {
        self.base
            .storage
            .put(resource, id, data, None, false, None)
            .await?;
        Ok(())
    }

    /// DELETE /resource/:id
    pub async fn delete(&self, resource: &str, id: &str) -> UnifiedStorageResult<bool> {
        let result = self.base.storage.delete(resource, id).await?;
        match result {
            UniversalResult::Count(c) => Ok(c > 0),
            _ => Ok(false),
        }
    }

    /// PATCH /resource/:id (partial update)
    pub async fn patch(
        &self,
        resource: &str,
        id: &str,
        updates: BTreeMap<String, UniversalValue>,
    ) -> UnifiedStorageResult<()> {
        let result = self.base.storage.get(resource, id).await?;

        if let UniversalResult::Record(record) = result {
            if let UniversalValue::Map(mut map) = record.value {
                for (field, value) in updates {
                    map.insert(field, value);
                }
                self.base
                    .storage
                    .put(resource, id, UniversalValue::Map(map), None, false, None)
                    .await?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl ProtocolAdapter for RestAdapter {
    fn protocol(&self) -> Protocol {
        Protocol::REST
    }

    async fn execute(
        &self,
        operation: UniversalOperation,
    ) -> UnifiedStorageResult<UniversalResult> {
        match operation {
            UniversalOperation::Get { namespace, key } => {
                self.base.storage.get(&namespace, &key).await
            }
            UniversalOperation::Put {
                namespace,
                key,
                value,
                ttl,
                if_not_exists,
                if_version,
            } => {
                self.base
                    .storage
                    .put(&namespace, &key, value, ttl, if_not_exists, if_version)
                    .await
            }
            UniversalOperation::Delete { namespace, key } => {
                self.base.storage.delete(&namespace, &key).await
            }
            _ => Err(UnifiedStorageError::NotImplemented(
                "Operation not supported".to_string(),
            )),
        }
    }

    fn to_universal(&self, data: &[u8]) -> UnifiedStorageResult<UniversalValue> {
        serde_json::from_slice(data)
            .map_err(|e| UnifiedStorageError::SerializationError(e.to_string()))
    }

    fn from_universal(&self, value: &UniversalValue) -> UnifiedStorageResult<Vec<u8>> {
        serde_json::to_vec(value)
            .map_err(|e| UnifiedStorageError::SerializationError(e.to_string()))
    }
}

// =============================================================================
// Adapter Factory
// =============================================================================

/// Factory for creating protocol adapters
pub struct AdapterFactory;

impl AdapterFactory {
    /// Create a Redis adapter
    pub fn redis(
        storage: Arc<UnifiedStorage>,
        schema_registry: Arc<SchemaRegistry>,
    ) -> RedisAdapter {
        RedisAdapter::new(storage, schema_registry)
    }

    /// Create a PostgreSQL adapter
    pub fn postgres(
        storage: Arc<UnifiedStorage>,
        schema_registry: Arc<SchemaRegistry>,
    ) -> SqlAdapter {
        SqlAdapter::postgres(storage, schema_registry)
    }

    /// Create a MySQL adapter
    pub fn mysql(storage: Arc<UnifiedStorage>, schema_registry: Arc<SchemaRegistry>) -> SqlAdapter {
        SqlAdapter::mysql(storage, schema_registry)
    }

    /// Create a CQL adapter
    pub fn cql(storage: Arc<UnifiedStorage>, schema_registry: Arc<SchemaRegistry>) -> CqlAdapter {
        CqlAdapter::new(storage, schema_registry)
    }

    /// Create a Cypher adapter
    pub fn cypher(
        storage: Arc<UnifiedStorage>,
        schema_registry: Arc<SchemaRegistry>,
    ) -> GraphAdapter {
        GraphAdapter::cypher(storage, schema_registry)
    }

    /// Create an AQL adapter
    pub fn aql(storage: Arc<UnifiedStorage>, schema_registry: Arc<SchemaRegistry>) -> GraphAdapter {
        GraphAdapter::aql(storage, schema_registry)
    }

    /// Create a REST adapter
    pub fn rest(storage: Arc<UnifiedStorage>, schema_registry: Arc<SchemaRegistry>) -> RestAdapter {
        RestAdapter::new(storage, schema_registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified::storage::{MemoryBackend, UnifiedStorageConfig};

    async fn setup() -> (Arc<UnifiedStorage>, Arc<SchemaRegistry>) {
        let backend = Arc::new(MemoryBackend::new());
        let config = UnifiedStorageConfig::default();
        let storage = Arc::new(UnifiedStorage::new(backend, config));
        storage.initialize().await.unwrap();
        let registry = Arc::new(SchemaRegistry::new());
        (storage, registry)
    }

    #[tokio::test]
    async fn test_redis_adapter_string_ops() {
        let (storage, registry) = setup().await;
        let adapter = RedisAdapter::new(storage, registry);

        // SET and GET
        adapter
            .set("mykey", UniversalValue::String("myvalue".to_string()))
            .await
            .unwrap();
        let value = adapter.get("mykey").await.unwrap();
        assert_eq!(value, Some(UniversalValue::String("myvalue".to_string())));

        // SETNX - should fail because key exists
        let result = adapter
            .setnx("mykey", UniversalValue::String("newvalue".to_string()))
            .await
            .unwrap();
        assert!(!result);

        // SETNX - should succeed for new key
        let result = adapter
            .setnx("newkey", UniversalValue::String("newvalue".to_string()))
            .await
            .unwrap();
        assert!(result);

        // DEL
        let deleted = adapter.del(&["mykey", "newkey"]).await.unwrap();
        assert_eq!(deleted, 2);

        // Verify deleted
        let value = adapter.get("mykey").await.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_redis_adapter_hash_ops() {
        let (storage, registry) = setup().await;
        let adapter = RedisAdapter::new(storage, registry);

        // HSET
        let created = adapter
            .hset(
                "myhash",
                "field1",
                UniversalValue::String("value1".to_string()),
            )
            .await
            .unwrap();
        assert!(created);

        // HGET
        let value = adapter.hget("myhash", "field1").await.unwrap();
        assert_eq!(value, Some(UniversalValue::String("value1".to_string())));

        // HINCRBY
        adapter
            .hset("myhash", "counter", UniversalValue::Int(0))
            .await
            .unwrap();
        let new_val = adapter.hincrby("myhash", "counter", 5).await.unwrap();
        assert_eq!(new_val, 5);

        // HGETALL
        let all = adapter.hgetall("myhash").await.unwrap();
        assert!(all.is_some());
        let map = all.unwrap();
        assert!(map.contains_key("field1"));
        assert!(map.contains_key("counter"));
    }

    #[tokio::test]
    async fn test_redis_adapter_list_ops() {
        let (storage, registry) = setup().await;
        let adapter = RedisAdapter::new(storage, registry);

        // LPUSH
        let len = adapter
            .lpush(
                "mylist",
                vec![
                    UniversalValue::String("a".to_string()),
                    UniversalValue::String("b".to_string()),
                ],
            )
            .await
            .unwrap();
        assert_eq!(len, 2);

        // RPUSH
        let len = adapter
            .rpush("mylist", vec![UniversalValue::String("c".to_string())])
            .await
            .unwrap();
        assert_eq!(len, 3);

        // LRANGE
        let values = adapter.lrange("mylist", 0, -1).await.unwrap();
        assert_eq!(values.len(), 3);

        // LPOP
        let popped = adapter.lpop("mylist", Some(1)).await.unwrap();
        assert_eq!(popped.len(), 1);

        // LLEN
        let len = adapter.llen("mylist").await.unwrap();
        assert_eq!(len, 2);
    }

    #[tokio::test]
    async fn test_sql_adapter_crud() {
        let (storage, registry) = setup().await;
        let adapter = SqlAdapter::postgres(storage, registry);

        // INSERT
        let mut row1 = BTreeMap::new();
        row1.insert("id".to_string(), UniversalValue::Int(1));
        row1.insert(
            "name".to_string(),
            UniversalValue::String("Alice".to_string()),
        );
        row1.insert("age".to_string(), UniversalValue::Int(30));
        adapter.insert("users", row1, "id").await.unwrap();

        let mut row2 = BTreeMap::new();
        row2.insert("id".to_string(), UniversalValue::Int(2));
        row2.insert(
            "name".to_string(),
            UniversalValue::String("Bob".to_string()),
        );
        row2.insert("age".to_string(), UniversalValue::Int(25));
        adapter.insert("users", row2, "id").await.unwrap();

        // SELECT all
        let rows = adapter
            .select("users", None, None, None, None, None)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);

        // SELECT with filter
        let filter = FilterExpression::Gt("age".to_string(), UniversalValue::Int(28));
        let rows = adapter
            .select("users", None, Some(filter), None, None, None)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].get("name"),
            Some(&UniversalValue::String("Alice".to_string()))
        );

        // UPDATE
        let mut updates = BTreeMap::new();
        updates.insert("age".to_string(), UniversalValue::Int(31));
        let filter = FilterExpression::Eq(
            "name".to_string(),
            UniversalValue::String("Alice".to_string()),
        );
        let count = adapter
            .update("users", updates, Some(filter))
            .await
            .unwrap();
        assert_eq!(count, 1);

        // COUNT
        let count = adapter.count("users", None).await.unwrap();
        assert_eq!(count, 2);

        // DELETE
        let filter = FilterExpression::Eq(
            "name".to_string(),
            UniversalValue::String("Bob".to_string()),
        );
        let count = adapter.delete("users", Some(filter)).await.unwrap();
        assert_eq!(count, 1);

        let count = adapter.count("users", None).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_cross_protocol_access() {
        let (storage, registry) = setup().await;

        // Write via Redis adapter
        let redis = RedisAdapter::new(Arc::clone(&storage), Arc::clone(&registry));
        redis
            .hset(
                "users:alice",
                "name",
                UniversalValue::String("Alice".to_string()),
            )
            .await
            .unwrap();
        redis
            .hset(
                "users:alice",
                "email",
                UniversalValue::String("alice@example.com".to_string()),
            )
            .await
            .unwrap();

        // Read via SQL adapter (same data!)
        let sql = SqlAdapter::postgres(Arc::clone(&storage), Arc::clone(&registry));
        let rows = sql
            .select("users", None, None, None, None, None)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(
            row.get("name"),
            Some(&UniversalValue::String("Alice".to_string()))
        );
        assert_eq!(
            row.get("email"),
            Some(&UniversalValue::String("alice@example.com".to_string()))
        );

        // Write via SQL, read via REST
        let mut bob = BTreeMap::new();
        bob.insert("id".to_string(), UniversalValue::String("bob".to_string()));
        bob.insert(
            "name".to_string(),
            UniversalValue::String("Bob".to_string()),
        );
        sql.insert("users", bob, "id").await.unwrap();

        let rest = RestAdapter::new(Arc::clone(&storage), Arc::clone(&registry));
        let users = rest.list("users", None, None, None).await.unwrap();
        assert_eq!(users.len(), 2);
    }

    #[tokio::test]
    async fn test_graph_adapter() {
        let (storage, registry) = setup().await;
        let adapter = GraphAdapter::cypher(storage, registry);

        // Create nodes
        let mut props = BTreeMap::new();
        props.insert(
            "name".to_string(),
            UniversalValue::String("Alice".to_string()),
        );
        adapter
            .create_node("n1", vec!["Person".to_string()], props.clone())
            .await
            .unwrap();

        props.insert(
            "name".to_string(),
            UniversalValue::String("Bob".to_string()),
        );
        adapter
            .create_node("n2", vec!["Person".to_string()], props)
            .await
            .unwrap();

        // Create relationship
        let edge_props = BTreeMap::new();
        adapter
            .create_relationship("e1", "KNOWS", "n1", "n2", edge_props)
            .await
            .unwrap();

        // Query node
        let node = adapter.get_node("n1").await.unwrap();
        assert!(node.is_some());

        // Query by label
        let persons = adapter.get_nodes_by_label("Person").await.unwrap();
        assert_eq!(persons.len(), 2);

        // Query relationships
        let rels = adapter
            .get_relationships("n1", Some("outgoing"), None)
            .await
            .unwrap();
        assert_eq!(rels.len(), 1);
    }

    #[tokio::test]
    async fn test_rest_adapter() {
        let (storage, registry) = setup().await;
        let adapter = RestAdapter::new(storage, registry);

        // POST (create)
        let mut user = BTreeMap::new();
        user.insert(
            "name".to_string(),
            UniversalValue::String("Alice".to_string()),
        );
        user.insert(
            "email".to_string(),
            UniversalValue::String("alice@example.com".to_string()),
        );
        adapter
            .create("users", "1", UniversalValue::Map(user))
            .await
            .unwrap();

        // GET
        let user = adapter.get("users", "1").await.unwrap();
        assert!(user.is_some());

        // LIST
        let users = adapter.list("users", None, None, None).await.unwrap();
        assert_eq!(users.len(), 1);

        // PATCH
        let mut updates = BTreeMap::new();
        updates.insert(
            "email".to_string(),
            UniversalValue::String("alice@newmail.com".to_string()),
        );
        adapter.patch("users", "1", updates).await.unwrap();

        let user = adapter.get("users", "1").await.unwrap().unwrap();
        if let UniversalValue::Map(map) = user {
            assert_eq!(
                map.get("email"),
                Some(&UniversalValue::String("alice@newmail.com".to_string()))
            );
        }

        // DELETE
        let deleted = adapter.delete("users", "1").await.unwrap();
        assert!(deleted);

        let user = adapter.get("users", "1").await.unwrap();
        assert!(user.is_none());
    }
}
