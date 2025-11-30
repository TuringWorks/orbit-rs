//! Redis Pub/Sub RESP commands
//!
//! Implements Redis publish/subscribe messaging:
//! - PUBLISH channel message - Publish a message to a channel
//! - SUBSCRIBE channel [channel ...] - Subscribe to channels
//! - UNSUBSCRIBE [channel ...] - Unsubscribe from channels
//! - PSUBSCRIBE pattern [pattern ...] - Subscribe to channels matching patterns
//! - PUNSUBSCRIBE [pattern ...] - Unsubscribe from pattern subscriptions
//! - PUBSUB subcommand [args] - Introspection commands

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::{error::ProtocolResult, resp::RespValue};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Pub/Sub channel state
struct Channel {
    /// Set of subscriber IDs
    subscribers: HashSet<String>,
    /// Messages waiting to be delivered
    pending_messages: Vec<String>,
}

impl Channel {
    fn new() -> Self {
        Self {
            subscribers: HashSet::new(),
            pending_messages: Vec::new(),
        }
    }
}

/// Pattern subscription info
struct PatternSubscription {
    pattern: String,
    subscriber_id: String,
}

/// Global pub/sub state manager
pub struct PubSubManager {
    /// Channel name -> Channel state
    channels: RwLock<HashMap<String, Channel>>,
    /// Pattern subscriptions
    pattern_subscriptions: RwLock<Vec<PatternSubscription>>,
    /// Subscriber ID -> subscribed channels
    subscriber_channels: RwLock<HashMap<String, HashSet<String>>>,
    /// Subscriber ID -> subscribed patterns
    subscriber_patterns: RwLock<HashMap<String, HashSet<String>>>,
}

impl PubSubManager {
    pub fn new() -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            pattern_subscriptions: RwLock::new(Vec::new()),
            subscriber_channels: RwLock::new(HashMap::new()),
            subscriber_patterns: RwLock::new(HashMap::new()),
        }
    }

    /// Publish a message to a channel
    pub async fn publish(&self, channel: &str, message: &str) -> usize {
        let mut channels = self.channels.write().await;
        let patterns = self.pattern_subscriptions.read().await;

        let mut subscriber_count = 0;

        // Direct channel subscribers
        if let Some(ch) = channels.get_mut(channel) {
            subscriber_count = ch.subscribers.len();
            ch.pending_messages.push(message.to_string());
        }

        // Pattern subscribers
        for pattern_sub in patterns.iter() {
            if Self::pattern_matches(&pattern_sub.pattern, channel) {
                subscriber_count += 1;
            }
        }

        debug!(
            "Published message to channel '{}', {} subscribers",
            channel, subscriber_count
        );

        subscriber_count
    }

    /// Subscribe to a channel
    pub async fn subscribe(&self, subscriber_id: &str, channel: &str) -> usize {
        let mut channels = self.channels.write().await;
        let mut subscriber_channels = self.subscriber_channels.write().await;

        // Get or create channel
        let ch = channels
            .entry(channel.to_string())
            .or_insert_with(Channel::new);
        ch.subscribers.insert(subscriber_id.to_string());

        // Track subscriber's channels
        subscriber_channels
            .entry(subscriber_id.to_string())
            .or_insert_with(HashSet::new)
            .insert(channel.to_string());

        let count = subscriber_channels
            .get(subscriber_id)
            .map(|s| s.len())
            .unwrap_or(0);

        info!(
            "Subscriber '{}' subscribed to channel '{}', total subscriptions: {}",
            subscriber_id, channel, count
        );

        count
    }

    /// Unsubscribe from a channel
    pub async fn unsubscribe(
        &self,
        subscriber_id: &str,
        channel: Option<&str>,
    ) -> Vec<(String, usize)> {
        let mut channels = self.channels.write().await;
        let mut subscriber_channels = self.subscriber_channels.write().await;

        let mut results = Vec::new();

        if let Some(channel_name) = channel {
            // Unsubscribe from specific channel
            if let Some(ch) = channels.get_mut(channel_name) {
                ch.subscribers.remove(subscriber_id);
            }
            if let Some(sub_channels) = subscriber_channels.get_mut(subscriber_id) {
                sub_channels.remove(channel_name);
            }
            let count = subscriber_channels
                .get(subscriber_id)
                .map(|s| s.len())
                .unwrap_or(0);
            results.push((channel_name.to_string(), count));
        } else {
            // Unsubscribe from all channels
            if let Some(sub_channels) = subscriber_channels.remove(subscriber_id) {
                for channel_name in sub_channels {
                    if let Some(ch) = channels.get_mut(&channel_name) {
                        ch.subscribers.remove(subscriber_id);
                    }
                    results.push((channel_name, 0));
                }
            }
        }

        results
    }

    /// Subscribe to a pattern
    pub async fn psubscribe(&self, subscriber_id: &str, pattern: &str) -> usize {
        let mut patterns = self.pattern_subscriptions.write().await;
        let mut subscriber_patterns = self.subscriber_patterns.write().await;

        patterns.push(PatternSubscription {
            pattern: pattern.to_string(),
            subscriber_id: subscriber_id.to_string(),
        });

        subscriber_patterns
            .entry(subscriber_id.to_string())
            .or_insert_with(HashSet::new)
            .insert(pattern.to_string());

        let count = subscriber_patterns
            .get(subscriber_id)
            .map(|s| s.len())
            .unwrap_or(0);

        info!(
            "Subscriber '{}' subscribed to pattern '{}', total patterns: {}",
            subscriber_id, pattern, count
        );

        count
    }

    /// Unsubscribe from a pattern
    pub async fn punsubscribe(
        &self,
        subscriber_id: &str,
        pattern: Option<&str>,
    ) -> Vec<(String, usize)> {
        let mut patterns = self.pattern_subscriptions.write().await;
        let mut subscriber_patterns = self.subscriber_patterns.write().await;

        let mut results = Vec::new();

        if let Some(pat) = pattern {
            // Unsubscribe from specific pattern
            patterns.retain(|p| !(p.pattern == pat && p.subscriber_id == subscriber_id));
            if let Some(sub_patterns) = subscriber_patterns.get_mut(subscriber_id) {
                sub_patterns.remove(pat);
            }
            let count = subscriber_patterns
                .get(subscriber_id)
                .map(|s| s.len())
                .unwrap_or(0);
            results.push((pat.to_string(), count));
        } else {
            // Unsubscribe from all patterns
            if let Some(sub_patterns) = subscriber_patterns.remove(subscriber_id) {
                for pat in sub_patterns {
                    patterns.retain(|p| !(p.pattern == pat && p.subscriber_id == subscriber_id));
                    results.push((pat, 0));
                }
            }
        }

        results
    }

    /// Get number of subscribers for a channel
    pub async fn numsub(&self, channel: &str) -> usize {
        let channels = self.channels.read().await;
        channels
            .get(channel)
            .map(|ch| ch.subscribers.len())
            .unwrap_or(0)
    }

    /// Get number of pattern subscriptions
    pub async fn numpat(&self) -> usize {
        let patterns = self.pattern_subscriptions.read().await;
        patterns.len()
    }

    /// Get all active channels
    pub async fn channels(&self, pattern: Option<&str>) -> Vec<String> {
        let channels = self.channels.read().await;
        channels
            .iter()
            .filter(|(name, ch)| {
                !ch.subscribers.is_empty()
                    && pattern
                        .map(|p| Self::pattern_matches(p, name))
                        .unwrap_or(true)
            })
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Check if a Redis GLOB-style pattern matches a string
    fn pattern_matches(pattern: &str, text: &str) -> bool {
        let mut pattern_chars = pattern.chars().peekable();
        let mut text_chars = text.chars().peekable();

        while let Some(p) = pattern_chars.next() {
            match p {
                '*' => {
                    // Match zero or more characters
                    if pattern_chars.peek().is_none() {
                        return true; // Trailing * matches everything
                    }
                    // Try matching at each position
                    while text_chars.peek().is_some() {
                        let remaining_pattern: String = pattern_chars.clone().collect();
                        let remaining_text: String = text_chars.clone().collect();
                        if Self::pattern_matches(&remaining_pattern, &remaining_text) {
                            return true;
                        }
                        text_chars.next();
                    }
                    let remaining_pattern: String = pattern_chars.collect();
                    return Self::pattern_matches(&remaining_pattern, "");
                }
                '?' => {
                    // Match exactly one character
                    if text_chars.next().is_none() {
                        return false;
                    }
                }
                '[' => {
                    // Character class
                    let mut negate = false;
                    let mut chars_to_match = Vec::new();

                    if pattern_chars.peek() == Some(&'^') || pattern_chars.peek() == Some(&'!') {
                        negate = true;
                        pattern_chars.next();
                    }

                    while let Some(&c) = pattern_chars.peek() {
                        if c == ']' {
                            pattern_chars.next();
                            break;
                        }
                        chars_to_match.push(c);
                        pattern_chars.next();
                    }

                    let text_char = match text_chars.next() {
                        Some(c) => c,
                        None => return false,
                    };

                    let matched = chars_to_match.contains(&text_char);
                    if (matched && negate) || (!matched && !negate) {
                        return false;
                    }
                }
                '\\' => {
                    // Escape next character
                    let escaped = pattern_chars.next().unwrap_or(p);
                    if text_chars.next() != Some(escaped) {
                        return false;
                    }
                }
                _ => {
                    // Literal character match
                    if text_chars.next() != Some(p) {
                        return false;
                    }
                }
            }
        }

        // Pattern exhausted, check if text is also exhausted
        text_chars.peek().is_none()
    }
}

impl Default for PubSubManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global pub/sub manager instance
static PUBSUB_MANAGER: std::sync::OnceLock<Arc<PubSubManager>> = std::sync::OnceLock::new();

fn get_pubsub_manager() -> &'static Arc<PubSubManager> {
    PUBSUB_MANAGER.get_or_init(|| Arc::new(PubSubManager::new()))
}

/// Handler for pub/sub commands
pub struct PubSubCommands {
    #[allow(dead_code)]
    base: BaseCommandHandler,
    /// Unique subscriber ID for this connection
    subscriber_id: String,
}

impl PubSubCommands {
    pub fn new(
        orbit_client: Arc<orbit_client::OrbitClient>,
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
            subscriber_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    async fn cmd_publish(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        self.validate_arg_count("PUBLISH", args, 2)?;

        let channel = self.get_string_arg(args, 0, "PUBLISH")?;
        let message = self.get_string_arg(args, 1, "PUBLISH")?;

        let manager = get_pubsub_manager();
        let subscriber_count = manager.publish(&channel, &message).await;

        debug!(
            "PUBLISH {} {} -> {} subscribers",
            channel, message, subscriber_count
        );
        Ok(RespValue::integer(subscriber_count as i64))
    }

    async fn cmd_subscribe(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'subscribe' command".to_string(),
            ));
        }

        let manager = get_pubsub_manager();
        let mut responses = Vec::new();

        for arg in args {
            let channel = arg.as_string().ok_or_else(|| {
                crate::protocols::error::ProtocolError::RespError(
                    "ERR invalid channel name".to_string(),
                )
            })?;

            let count = manager.subscribe(&self.subscriber_id, &channel).await;

            responses.push(RespValue::array(vec![
                RespValue::bulk_string_from_str("subscribe"),
                RespValue::bulk_string_from_str(&channel),
                RespValue::integer(count as i64),
            ]));
        }

        // Return array of subscription confirmations
        if responses.len() == 1 {
            Ok(responses.into_iter().next().unwrap())
        } else {
            Ok(RespValue::array(responses))
        }
    }

    async fn cmd_unsubscribe(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        let manager = get_pubsub_manager();

        let results = if args.is_empty() {
            // Unsubscribe from all channels
            manager.unsubscribe(&self.subscriber_id, None).await
        } else {
            // Unsubscribe from specified channels
            let mut all_results = Vec::new();
            for arg in args {
                if let Some(channel) = arg.as_string() {
                    let results = manager
                        .unsubscribe(&self.subscriber_id, Some(&channel))
                        .await;
                    all_results.extend(results);
                }
            }
            all_results
        };

        let mut responses = Vec::new();
        for (channel, count) in results {
            responses.push(RespValue::array(vec![
                RespValue::bulk_string_from_str("unsubscribe"),
                RespValue::bulk_string_from_str(&channel),
                RespValue::integer(count as i64),
            ]));
        }

        if responses.is_empty() {
            Ok(RespValue::array(vec![
                RespValue::bulk_string_from_str("unsubscribe"),
                RespValue::null(),
                RespValue::integer(0),
            ]))
        } else if responses.len() == 1 {
            Ok(responses.into_iter().next().unwrap())
        } else {
            Ok(RespValue::array(responses))
        }
    }

    async fn cmd_psubscribe(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'psubscribe' command".to_string(),
            ));
        }

        let manager = get_pubsub_manager();
        let mut responses = Vec::new();

        for arg in args {
            let pattern = arg.as_string().ok_or_else(|| {
                crate::protocols::error::ProtocolError::RespError("ERR invalid pattern".to_string())
            })?;

            let count = manager.psubscribe(&self.subscriber_id, &pattern).await;

            responses.push(RespValue::array(vec![
                RespValue::bulk_string_from_str("psubscribe"),
                RespValue::bulk_string_from_str(&pattern),
                RespValue::integer(count as i64),
            ]));
        }

        if responses.len() == 1 {
            Ok(responses.into_iter().next().unwrap())
        } else {
            Ok(RespValue::array(responses))
        }
    }

    async fn cmd_punsubscribe(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        let manager = get_pubsub_manager();

        let results = if args.is_empty() {
            // Unsubscribe from all patterns
            manager.punsubscribe(&self.subscriber_id, None).await
        } else {
            // Unsubscribe from specified patterns
            let mut all_results = Vec::new();
            for arg in args {
                if let Some(pattern) = arg.as_string() {
                    let results = manager
                        .punsubscribe(&self.subscriber_id, Some(&pattern))
                        .await;
                    all_results.extend(results);
                }
            }
            all_results
        };

        let mut responses = Vec::new();
        for (pattern, count) in results {
            responses.push(RespValue::array(vec![
                RespValue::bulk_string_from_str("punsubscribe"),
                RespValue::bulk_string_from_str(&pattern),
                RespValue::integer(count as i64),
            ]));
        }

        if responses.is_empty() {
            Ok(RespValue::array(vec![
                RespValue::bulk_string_from_str("punsubscribe"),
                RespValue::null(),
                RespValue::integer(0),
            ]))
        } else if responses.len() == 1 {
            Ok(responses.into_iter().next().unwrap())
        } else {
            Ok(RespValue::array(responses))
        }
    }

    async fn cmd_pubsub(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(crate::protocols::error::ProtocolError::RespError(
                "ERR wrong number of arguments for 'pubsub' command".to_string(),
            ));
        }

        let subcommand = self.get_string_arg(args, 0, "PUBSUB")?.to_uppercase();
        let manager = get_pubsub_manager();

        match subcommand.as_str() {
            "CHANNELS" => {
                let pattern = args.get(1).and_then(|v| v.as_string());
                let channels = manager.channels(pattern.as_deref()).await;
                Ok(RespValue::array(
                    channels
                        .into_iter()
                        .map(|c| RespValue::bulk_string_from_str(&c))
                        .collect(),
                ))
            }
            "NUMSUB" => {
                let mut results = Vec::new();
                for arg in args.iter().skip(1) {
                    if let Some(channel) = arg.as_string() {
                        let count = manager.numsub(&channel).await;
                        results.push(RespValue::bulk_string_from_str(&channel));
                        results.push(RespValue::integer(count as i64));
                    }
                }
                Ok(RespValue::array(results))
            }
            "NUMPAT" => {
                let count = manager.numpat().await;
                Ok(RespValue::integer(count as i64))
            }
            _ => Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR Unknown PUBSUB subcommand '{}'",
                subcommand
            ))),
        }
    }
}

#[async_trait]
impl CommandHandler for PubSubCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command_name {
            "PUBLISH" => self.cmd_publish(args).await,
            "SUBSCRIBE" => self.cmd_subscribe(args).await,
            "UNSUBSCRIBE" => self.cmd_unsubscribe(args).await,
            "PSUBSCRIBE" => self.cmd_psubscribe(args).await,
            "PUNSUBSCRIBE" => self.cmd_punsubscribe(args).await,
            "PUBSUB" => self.cmd_pubsub(args).await,
            _ => Err(crate::protocols::error::ProtocolError::RespError(format!(
                "ERR unknown pubsub command '{command_name}'"
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &[
            "PUBLISH",
            "SUBSCRIBE",
            "UNSUBSCRIBE",
            "PSUBSCRIBE",
            "PUNSUBSCRIBE",
            "PUBSUB",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        // Simple patterns
        assert!(PubSubManager::pattern_matches("hello", "hello"));
        assert!(!PubSubManager::pattern_matches("hello", "world"));

        // Wildcard *
        assert!(PubSubManager::pattern_matches("h*o", "hello"));
        assert!(PubSubManager::pattern_matches("h*", "hello"));
        assert!(PubSubManager::pattern_matches("*llo", "hello"));
        assert!(PubSubManager::pattern_matches("*", "anything"));
        assert!(PubSubManager::pattern_matches("news.*", "news.sports"));
        assert!(PubSubManager::pattern_matches("news.*", "news.tech.ai"));

        // Single character ?
        assert!(PubSubManager::pattern_matches("h?llo", "hello"));
        assert!(PubSubManager::pattern_matches("h?llo", "hallo"));
        assert!(!PubSubManager::pattern_matches("h?llo", "heello"));

        // Character class []
        assert!(PubSubManager::pattern_matches("h[ae]llo", "hello"));
        assert!(PubSubManager::pattern_matches("h[ae]llo", "hallo"));
        assert!(!PubSubManager::pattern_matches("h[ae]llo", "hillo"));

        // Negated character class
        assert!(PubSubManager::pattern_matches("h[!a]llo", "hello"));
        assert!(!PubSubManager::pattern_matches("h[!e]llo", "hello"));
    }

    #[tokio::test]
    async fn test_pubsub_manager() {
        let manager = PubSubManager::new();

        // Subscribe
        let count = manager.subscribe("sub1", "channel1").await;
        assert_eq!(count, 1);

        let count = manager.subscribe("sub1", "channel2").await;
        assert_eq!(count, 2);

        // Publish
        let subs = manager.publish("channel1", "message1").await;
        assert_eq!(subs, 1);

        // Numsub
        let numsub = manager.numsub("channel1").await;
        assert_eq!(numsub, 1);

        // Channels
        let channels = manager.channels(None).await;
        assert_eq!(channels.len(), 2);

        // Unsubscribe
        let results = manager.unsubscribe("sub1", Some("channel1")).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 1); // 1 remaining subscription
    }

    #[tokio::test]
    async fn test_pattern_subscriptions() {
        let manager = PubSubManager::new();

        // Pattern subscribe
        let count = manager.psubscribe("sub1", "news.*").await;
        assert_eq!(count, 1);

        // Publish to matching channel
        let subs = manager.publish("news.sports", "game").await;
        assert_eq!(subs, 1);

        // Numpat
        let numpat = manager.numpat().await;
        assert_eq!(numpat, 1);

        // Punsubscribe
        let results = manager.punsubscribe("sub1", Some("news.*")).await;
        assert_eq!(results.len(), 1);
    }
}
