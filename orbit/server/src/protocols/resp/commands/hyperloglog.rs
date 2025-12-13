//! HyperLogLog command handlers for Redis RESP protocol
//!
//! This module implements HyperLogLog commands (PFADD, PFCOUNT, PFMERGE)
//! for probabilistic cardinality estimation.
//!
//! HyperLogLog is a probabilistic data structure used to estimate the
//! cardinality of a set with ~0.81% standard error using only 12KB of memory.
//!
//! ## References
//! - Redis HyperLogLog: https://redis.io/docs/data-types/probabilistic/hyperloglogs/
//! - HyperLogLog Paper: http://algo.inria.fr/flajolet/Publications/FlFuGaMe07.pdf

use super::traits::{BaseCommandHandler, CommandHandler};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::resp::RespValue;
use async_trait::async_trait;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tracing::debug;

/// HyperLogLog precision (number of registers)
/// Redis uses 2^14 = 16384 registers
const HLL_REGISTERS: usize = 16384;
const HLL_P: u32 = 14; // log2(HLL_REGISTERS)

/// HyperLogLog data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLogLog {
    /// Registers for storing maximum leading zeros
    registers: Vec<u8>,
}

impl HyperLogLog {
    /// Create a new empty HyperLogLog
    pub fn new() -> Self {
        Self {
            registers: vec![0; HLL_REGISTERS],
        }
    }

    /// Add an element to the HyperLogLog
    pub fn add(&mut self, element: &[u8]) -> bool {
        let hash = Self::hash_bytes(element);

        // Extract register index from first P bits
        let register_index = (hash & ((1 << HLL_P) - 1)) as usize;

        // Get remaining bits and count leading zeros + 1
        let remaining = hash >> HLL_P;
        let leading_zeros = if remaining == 0 {
            (64 - HLL_P + 1) as u8
        } else {
            (remaining.leading_zeros() + 1 - HLL_P) as u8
        };

        // Update register if new value is larger
        let old_value = self.registers[register_index];
        if leading_zeros > old_value {
            self.registers[register_index] = leading_zeros;
            return true; // Register was updated
        }
        false
    }

    /// Estimate the cardinality
    pub fn count(&self) -> u64 {
        let m = HLL_REGISTERS as f64;
        let alpha = Self::alpha(m);

        // Calculate raw estimate
        let mut sum = 0.0;
        let mut zeros = 0;

        for &register in &self.registers {
            sum += 2.0_f64.powi(-(register as i32));
            if register == 0 {
                zeros += 1;
            }
        }

        let raw_estimate = alpha * m * m / sum;

        // Apply bias correction
        let estimate = if raw_estimate <= 5.0 * m {
            // Small range correction
            if zeros > 0 {
                m * (m / zeros as f64).ln()
            } else {
                raw_estimate
            }
        } else if raw_estimate <= (1.0 / 30.0) * (1u64 << 32) as f64 {
            // No correction
            raw_estimate
        } else {
            // Large range correction
            let two_32 = (1u64 << 32) as f64;
            -two_32 * (1.0 - raw_estimate / two_32).ln()
        };

        estimate as u64
    }

    /// Merge another HyperLogLog into this one
    pub fn merge(&mut self, other: &HyperLogLog) {
        for i in 0..HLL_REGISTERS {
            if other.registers[i] > self.registers[i] {
                self.registers[i] = other.registers[i];
            }
        }
    }

    /// Hash bytes using a fast hash function
    fn hash_bytes(bytes: &[u8]) -> u64 {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    }

    /// Calculate alpha constant for HyperLogLog
    fn alpha(m: f64) -> f64 {
        match m as usize {
            16 => 0.673,
            32 => 0.697,
            64 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / m),
        }
    }

    /// Serialize to base64-encoded string for storage
    pub fn to_string(&self) -> String {
        let bytes = bincode::serialize(&self).unwrap_or_default();
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    /// Deserialize from base64-encoded string
    pub fn from_string(s: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(s)?;
        Ok(bincode::deserialize(&bytes)?)
    }
}

impl Default for HyperLogLog {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HyperLogLogCommands {
    base: BaseCommandHandler,
}

impl HyperLogLogCommands {
    pub fn new(
        orbit_client: Arc<orbit_client::OrbitClient>,
        local_registry: Arc<crate::protocols::resp::simple_local::SimpleLocalRegistry>,
    ) -> Self {
        Self {
            base: BaseCommandHandler::new(orbit_client, local_registry),
        }
    }

    fn get_string_arg(
        &self,
        args: &[RespValue],
        index: usize,
        command_name: &str,
    ) -> ProtocolResult<String> {
        args.get(index).and_then(|v| v.as_string()).ok_or_else(|| {
            ProtocolError::RespError(format!(
                "ERR invalid argument for '{}' command",
                command_name.to_lowercase()
            ))
        })
    }

    /// PFADD key element [element ...]
    /// Adds elements to a HyperLogLog data structure
    async fn cmd_pfadd(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'pfadd' command".to_string(),
            ));
        }

        let key = self.get_string_arg(args, 0, "pfadd")?;

        // Get existing HLL or create new
        let result = self
            .base
            .local_registry
            .execute_keyvalue(&key, "get_value", &[])
            .await
            .ok();

        let mut hll = if let Some(value_json) = result {
            if let Ok(Some(encoded_str)) = serde_json::from_value::<Option<String>>(value_json) {
                HyperLogLog::from_string(&encoded_str).unwrap_or_else(|_| HyperLogLog::new())
            } else {
                HyperLogLog::new()
            }
        } else {
            HyperLogLog::new()
        };

        // Add all elements
        let mut modified = false;
        for i in 1..args.len() {
            if let Some(element_str) = args[i].as_string() {
                if hll.add(element_str.as_bytes()) {
                    modified = true;
                }
            }
        }

        // Store updated HLL
        if modified {
            let hll_str = hll.to_string();
            self.base
                .local_registry
                .execute_keyvalue(&key, "set_value", &[serde_json::to_value(hll_str)?])
                .await
                .map_err(|e| ProtocolError::RespError(format!("ERR failed to store HLL: {}", e)))?;
        }

        // Return 1 if modified, 0 otherwise
        Ok(RespValue::Integer(if modified { 1 } else { 0 }))
    }

    /// PFCOUNT key [key ...]
    /// Returns the approximated cardinality of the set(s)
    async fn cmd_pfcount(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'pfcount' command".to_string(),
            ));
        }

        if args.len() == 1 {
            // Single key
            let key = self.get_string_arg(args, 0, "pfcount")?;

            let result = self
                .base
                .local_registry
                .execute_keyvalue(&key, "get_value", &[])
                .await
                .ok();

            let hll = if let Some(value_json) = result {
                if let Ok(Some(encoded_str)) = serde_json::from_value::<Option<String>>(value_json)
                {
                    HyperLogLog::from_string(&encoded_str).unwrap_or_else(|_| HyperLogLog::new())
                } else {
                    HyperLogLog::new()
                }
            } else {
                HyperLogLog::new()
            };

            Ok(RespValue::Integer(hll.count() as i64))
        } else {
            // Multiple keys - merge HLLs
            let mut merged = HyperLogLog::new();

            for arg in args {
                if let Some(key) = arg.as_string() {
                    let result = self
                        .base
                        .local_registry
                        .execute_keyvalue(&key, "get_value", &[])
                        .await
                        .ok();

                    if let Some(value_json) = result {
                        if let Ok(Some(encoded_str)) =
                            serde_json::from_value::<Option<String>>(value_json)
                        {
                            if let Ok(hll) = HyperLogLog::from_string(&encoded_str) {
                                merged.merge(&hll);
                            }
                        }
                    }
                }
            }

            Ok(RespValue::Integer(merged.count() as i64))
        }
    }

    /// PFMERGE destkey sourcekey [sourcekey ...]
    /// Merge multiple HyperLogLog values into a unique value
    async fn cmd_pfmerge(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'pfmerge' command".to_string(),
            ));
        }

        let dest_key = self.get_string_arg(args, 0, "pfmerge")?;

        let mut merged = HyperLogLog::new();

        // Merge all source HLLs
        for i in 1..args.len() {
            if let Some(source_key) = args[i].as_string() {
                let result = self
                    .base
                    .local_registry
                    .execute_keyvalue(&source_key, "get_value", &[])
                    .await
                    .ok();

                if let Some(value_json) = result {
                    if let Ok(Some(encoded_str)) =
                        serde_json::from_value::<Option<String>>(value_json)
                    {
                        if let Ok(hll) = HyperLogLog::from_string(&encoded_str) {
                            merged.merge(&hll);
                        }
                    }
                }
            }
        }

        // Store merged result
        let merged_str = merged.to_string();
        self.base
            .local_registry
            .execute_keyvalue(&dest_key, "set_value", &[serde_json::to_value(merged_str)?])
            .await
            .map_err(|e| {
                ProtocolError::RespError(format!("ERR failed to store merged HLL: {}", e))
            })?;

        Ok(RespValue::simple_string("OK"))
    }
}

#[async_trait]
impl CommandHandler for HyperLogLogCommands {
    async fn handle(&self, command_name: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        debug!("Executing HyperLogLog command: {}", command_name);

        match command_name.to_uppercase().as_str() {
            "PFADD" => self.cmd_pfadd(args).await,
            "PFCOUNT" => self.cmd_pfcount(args).await,
            "PFMERGE" => self.cmd_pfmerge(args).await,
            _ => Err(ProtocolError::RespError(format!(
                "ERR unknown HyperLogLog command '{}'",
                command_name
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &["PFADD", "PFCOUNT", "PFMERGE"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hyperloglog_add_and_count() {
        let mut hll = HyperLogLog::new();

        // Add some elements
        hll.add(b"apple");
        hll.add(b"banana");
        hll.add(b"cherry");
        hll.add(b"apple"); // Duplicate

        let count = hll.count();
        // Should estimate ~3 (allowing for error margin)
        assert!(count >= 2 && count <= 4, "Count was {}", count);
    }

    #[test]
    fn test_hyperloglog_large_set() {
        let mut hll = HyperLogLog::new();

        // Add 10,000 unique elements
        for i in 0..10000 {
            hll.add(format!("element_{}", i).as_bytes());
        }

        let count = hll.count();
        // Should be within ~0.81% error: 9919 to 10081
        assert!(count >= 9900 && count <= 10100, "Count was {}", count);
    }

    #[test]
    fn test_hyperloglog_merge() {
        let mut hll1 = HyperLogLog::new();
        let mut hll2 = HyperLogLog::new();

        // Add different elements to each
        for i in 0..5000 {
            hll1.add(format!("set1_{}", i).as_bytes());
        }
        for i in 0..5000 {
            hll2.add(format!("set2_{}", i).as_bytes());
        }

        // Merge
        hll1.merge(&hll2);

        let count = hll1.count();
        // Should estimate ~10,000
        assert!(count >= 9800 && count <= 10200, "Count was {}", count);
    }

    #[test]
    fn test_hyperloglog_serialization() {
        let mut hll = HyperLogLog::new();
        hll.add(b"test1");
        hll.add(b"test2");
        hll.add(b"test3");

        let encoded = hll.to_string();
        let deserialized = HyperLogLog::from_string(&encoded).unwrap();

        assert_eq!(hll.count(), deserialized.count());
    }
}
