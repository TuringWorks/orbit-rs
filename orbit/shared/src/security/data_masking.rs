//! Dynamic Data Masking
//!
//! Provides data masking capabilities for protecting sensitive information while
//! maintaining data utility. Unlike encryption, masking is typically one-way and
//! produces human-readable output suitable for analytics and reporting.
//!
//! # Features
//!
//! - **Partial Masking**: Show first/last N characters (e.g., `***-**-6789`)
//! - **Full Masking**: Replace entire value with mask character
//! - **Pattern Masking**: Apply masking based on patterns (credit cards, SSN, email)
//! - **Hash Masking**: Replace with consistent hash for anonymization
//! - **Shuffle Masking**: Randomize within column for statistical preservation
//! - **Redaction**: Complete removal of sensitive data
//! - **Tokenization**: Replace with reversible tokens (requires secure token store)
//!
//! # Usage
//!
//! ```rust,ignore
//! use orbit_shared::security::data_masking::{DataMaskingEngine, MaskingPolicy};
//!
//! let engine = DataMaskingEngine::new();
//!
//! // Define masking policy for a table
//! let policy = MaskingPolicy::new("users")
//!     .mask_field("ssn", MaskingStrategy::PartialMask { show_last: 4 })
//!     .mask_field("credit_card", MaskingStrategy::CreditCard)
//!     .mask_field("email", MaskingStrategy::Email)
//!     .mask_field("salary", MaskingStrategy::Redact);
//!
//! engine.register_policy(policy).await?;
//!
//! // Apply masking to row
//! let masked_row = engine.mask_row("users", &row, &user_context).await?;
//! ```

use crate::exception::{OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Masking strategy types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MaskingStrategy {
    /// Show only first N characters, mask the rest
    ShowFirst { count: usize, mask_char: char },

    /// Show only last N characters, mask the rest
    ShowLast { count: usize, mask_char: char },

    /// Show first and last N characters
    ShowFirstAndLast {
        first: usize,
        last: usize,
        mask_char: char,
    },

    /// Full masking - replace entire value
    FullMask { mask_char: char },

    /// Credit card masking (show last 4 digits)
    CreditCard,

    /// SSN masking (show last 4 digits)
    Ssn,

    /// Email masking (show first char and domain)
    Email,

    /// Phone number masking (show last 4 digits)
    Phone,

    /// Hash the value for consistent anonymization
    Hash { salt: Option<String> },

    /// Redact completely (replace with null or placeholder)
    Redact { replacement: Option<String> },

    /// Random shuffle within same column (preserves statistics)
    Shuffle,

    /// Range masking for numbers (e.g., 25000-30000)
    NumericRange { bucket_size: f64 },

    /// Date masking (generalize to month/year)
    DateGeneralize { precision: DatePrecision },

    /// Custom regex-based masking
    RegexMask {
        pattern: String,
        replacement: String,
    },

    /// Tokenization (reversible with token store)
    Tokenize { token_prefix: String },
}

/// Date precision for date masking
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DatePrecision {
    /// Keep only year
    Year,
    /// Keep year and month
    Month,
    /// Keep year, month, and day (no time)
    Day,
    /// Keep up to hour
    Hour,
}

/// Access level for masking decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AccessLevel {
    /// No access - full masking
    None,
    /// Limited access - partial masking
    Limited,
    /// Standard access - light masking
    Standard,
    /// Full access - no masking
    Full,
}

/// User context for masking decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskingContext {
    /// User's access level
    pub access_level: AccessLevel,
    /// User's roles
    pub roles: Vec<String>,
    /// User's department
    pub department: Option<String>,
    /// Whether user is data owner
    pub is_data_owner: bool,
    /// Custom attributes for policy evaluation
    pub attributes: HashMap<String, String>,
}

impl MaskingContext {
    /// Create a new masking context with access level
    pub fn new(access_level: AccessLevel) -> Self {
        Self {
            access_level,
            roles: Vec::new(),
            department: None,
            is_data_owner: false,
            attributes: HashMap::new(),
        }
    }

    /// Add roles
    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    /// Set department
    pub fn with_department(mut self, department: &str) -> Self {
        self.department = Some(department.to_string());
        self
    }

    /// Mark as data owner
    pub fn as_data_owner(mut self) -> Self {
        self.is_data_owner = true;
        self
    }

    /// Check if user has role
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

/// Configuration for masking a single field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMaskingConfig {
    /// Field name
    pub field_name: String,
    /// Default masking strategy
    pub default_strategy: MaskingStrategy,
    /// Access level overrides (higher access = less masking)
    pub access_overrides: HashMap<AccessLevel, Option<MaskingStrategy>>,
    /// Role-based overrides
    pub role_overrides: HashMap<String, Option<MaskingStrategy>>,
    /// Whether field is nullable after masking
    pub preserve_null: bool,
    /// Minimum access level to see any data
    pub min_access_level: AccessLevel,
}

impl FieldMaskingConfig {
    /// Create a new field masking config
    pub fn new(field_name: &str, strategy: MaskingStrategy) -> Self {
        Self {
            field_name: field_name.to_string(),
            default_strategy: strategy,
            access_overrides: HashMap::new(),
            role_overrides: HashMap::new(),
            preserve_null: true,
            min_access_level: AccessLevel::None,
        }
    }

    /// Add access level override
    pub fn with_access_override(
        mut self,
        level: AccessLevel,
        strategy: Option<MaskingStrategy>,
    ) -> Self {
        self.access_overrides.insert(level, strategy);
        self
    }

    /// Add role override
    pub fn with_role_override(mut self, role: &str, strategy: Option<MaskingStrategy>) -> Self {
        self.role_overrides.insert(role.to_string(), strategy);
        self
    }

    /// Set minimum access level
    pub fn with_min_access(mut self, level: AccessLevel) -> Self {
        self.min_access_level = level;
        self
    }

    /// Get effective strategy for context
    pub fn get_strategy_for_context(&self, ctx: &MaskingContext) -> Option<&MaskingStrategy> {
        // Data owners bypass masking
        if ctx.is_data_owner {
            return None;
        }

        // Check minimum access level
        if ctx.access_level < self.min_access_level {
            return Some(&MaskingStrategy::Redact { replacement: None });
        }

        // Check role overrides first (most specific)
        for role in &ctx.roles {
            if let Some(strategy) = self.role_overrides.get(role) {
                return strategy.as_ref();
            }
        }

        // Check access level overrides
        if let Some(strategy) = self.access_overrides.get(&ctx.access_level) {
            return strategy.as_ref();
        }

        // Return default strategy
        Some(&self.default_strategy)
    }
}

/// Masking policy for a table/collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskingPolicy {
    /// Table or collection name
    pub table_name: String,
    /// Field masking configurations
    pub field_configs: HashMap<String, FieldMaskingConfig>,
    /// Whether policy is enabled
    pub enabled: bool,
    /// Policy version
    pub version: u32,
    /// Policy description
    pub description: Option<String>,
}

impl MaskingPolicy {
    /// Create a new masking policy for a table
    pub fn new(table_name: &str) -> Self {
        Self {
            table_name: table_name.to_string(),
            field_configs: HashMap::new(),
            enabled: true,
            version: 1,
            description: None,
        }
    }

    /// Add a field with masking strategy
    pub fn mask_field(mut self, field_name: &str, strategy: MaskingStrategy) -> Self {
        let config = FieldMaskingConfig::new(field_name, strategy);
        self.field_configs.insert(field_name.to_string(), config);
        self
    }

    /// Add a field with full configuration
    pub fn mask_field_with_config(mut self, config: FieldMaskingConfig) -> Self {
        self.field_configs.insert(config.field_name.clone(), config);
        self
    }

    /// Set policy description
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    /// Check if a field has masking configured
    pub fn is_field_masked(&self, field_name: &str) -> bool {
        self.field_configs.contains_key(field_name)
    }
}

/// Token store for tokenization strategy
pub struct TokenStore {
    /// Token to original value mapping
    tokens: Arc<RwLock<HashMap<String, String>>>,
    /// Original to token mapping
    reverse: Arc<RwLock<HashMap<String, String>>>,
}

impl TokenStore {
    /// Create a new token store
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            reverse: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create or retrieve token for a value
    pub async fn tokenize(&self, value: &str, prefix: &str) -> String {
        let mut reverse = self.reverse.write().await;
        if let Some(existing) = reverse.get(value) {
            return existing.clone();
        }

        // Generate new token
        let token = format!(
            "{}_{}",
            prefix,
            uuid::Uuid::new_v4().to_string().replace("-", "")[..12].to_uppercase()
        );

        let mut tokens = self.tokens.write().await;
        tokens.insert(token.clone(), value.to_string());
        reverse.insert(value.to_string(), token.clone());

        token
    }

    /// Detokenize a token back to original value
    pub async fn detokenize(&self, token: &str) -> Option<String> {
        let tokens = self.tokens.read().await;
        tokens.get(token).cloned()
    }
}

impl Default for TokenStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Data Masking Engine
pub struct DataMaskingEngine {
    /// Policies indexed by table name
    policies: Arc<RwLock<HashMap<String, MaskingPolicy>>>,
    /// Token store for tokenization
    token_store: Arc<TokenStore>,
    /// Salt for hash masking
    hash_salt: String,
}

impl DataMaskingEngine {
    /// Create a new data masking engine
    pub fn new() -> Self {
        Self {
            policies: Arc::new(RwLock::new(HashMap::new())),
            token_store: Arc::new(TokenStore::new()),
            hash_salt: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Create with custom hash salt
    pub fn with_salt(salt: &str) -> Self {
        Self {
            policies: Arc::new(RwLock::new(HashMap::new())),
            token_store: Arc::new(TokenStore::new()),
            hash_salt: salt.to_string(),
        }
    }

    /// Register a masking policy
    pub async fn register_policy(&self, policy: MaskingPolicy) -> OrbitResult<()> {
        let mut policies = self.policies.write().await;
        policies.insert(policy.table_name.clone(), policy);
        Ok(())
    }

    /// Get policy for a table
    pub async fn get_policy(&self, table_name: &str) -> Option<MaskingPolicy> {
        let policies = self.policies.read().await;
        policies.get(table_name).cloned()
    }

    /// Get token store for detokenization
    pub fn token_store(&self) -> &Arc<TokenStore> {
        &self.token_store
    }

    /// Mask a single value according to strategy
    pub async fn mask_value(
        &self,
        value: &serde_json::Value,
        strategy: &MaskingStrategy,
    ) -> OrbitResult<serde_json::Value> {
        match value {
            serde_json::Value::Null => Ok(serde_json::Value::Null),
            serde_json::Value::String(s) => {
                let masked = self.mask_string(s, strategy).await?;
                Ok(serde_json::Value::String(masked))
            }
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    let masked = self.mask_number(f, strategy)?;
                    Ok(serde_json::json!(masked))
                } else {
                    Ok(value.clone())
                }
            }
            _ => Ok(value.clone()),
        }
    }

    /// Mask a string value
    async fn mask_string(&self, value: &str, strategy: &MaskingStrategy) -> OrbitResult<String> {
        match strategy {
            MaskingStrategy::ShowFirst { count, mask_char } => {
                Ok(self.show_first(value, *count, *mask_char))
            }
            MaskingStrategy::ShowLast { count, mask_char } => {
                Ok(self.show_last(value, *count, *mask_char))
            }
            MaskingStrategy::ShowFirstAndLast {
                first,
                last,
                mask_char,
            } => Ok(self.show_first_and_last(value, *first, *last, *mask_char)),
            MaskingStrategy::FullMask { mask_char } => {
                Ok(mask_char.to_string().repeat(value.len().min(10)))
            }
            MaskingStrategy::CreditCard => Ok(self.mask_credit_card(value)),
            MaskingStrategy::Ssn => Ok(self.mask_ssn(value)),
            MaskingStrategy::Email => Ok(self.mask_email(value)),
            MaskingStrategy::Phone => Ok(self.mask_phone(value)),
            MaskingStrategy::Hash { salt } => Ok(self.hash_value(value, salt.as_deref())),
            MaskingStrategy::Redact { replacement } => Ok(replacement
                .clone()
                .unwrap_or_else(|| "[REDACTED]".to_string())),
            MaskingStrategy::Tokenize { token_prefix } => {
                Ok(self.token_store.tokenize(value, token_prefix).await)
            }
            MaskingStrategy::RegexMask {
                pattern,
                replacement,
            } => self.regex_mask(value, pattern, replacement),
            MaskingStrategy::Shuffle => {
                // For single value, shuffle doesn't apply - return as-is
                Ok(value.to_string())
            }
            MaskingStrategy::NumericRange { .. } => {
                // Not applicable to strings
                Ok(value.to_string())
            }
            MaskingStrategy::DateGeneralize { precision } => {
                self.generalize_date(value, *precision)
            }
        }
    }

    /// Mask a numeric value
    fn mask_number(&self, value: f64, strategy: &MaskingStrategy) -> OrbitResult<String> {
        match strategy {
            MaskingStrategy::NumericRange { bucket_size } => {
                let lower = (value / bucket_size).floor() * bucket_size;
                let upper = lower + bucket_size;
                Ok(format!("{}-{}", lower as i64, upper as i64))
            }
            MaskingStrategy::Redact { replacement } => Ok(replacement
                .clone()
                .unwrap_or_else(|| "[REDACTED]".to_string())),
            MaskingStrategy::Hash { salt } => {
                Ok(self.hash_value(&value.to_string(), salt.as_deref()))
            }
            _ => Ok(value.to_string()),
        }
    }

    /// Show first N characters
    fn show_first(&self, value: &str, count: usize, mask_char: char) -> String {
        let chars: Vec<char> = value.chars().collect();
        if chars.len() <= count {
            return value.to_string();
        }
        let visible: String = chars[..count].iter().collect();
        let masked = mask_char.to_string().repeat(chars.len() - count);
        format!("{}{}", visible, masked)
    }

    /// Show last N characters
    fn show_last(&self, value: &str, count: usize, mask_char: char) -> String {
        let chars: Vec<char> = value.chars().collect();
        if chars.len() <= count {
            return value.to_string();
        }
        let masked = mask_char.to_string().repeat(chars.len() - count);
        let visible: String = chars[chars.len() - count..].iter().collect();
        format!("{}{}", masked, visible)
    }

    /// Show first and last N characters
    fn show_first_and_last(
        &self,
        value: &str,
        first: usize,
        last: usize,
        mask_char: char,
    ) -> String {
        let chars: Vec<char> = value.chars().collect();
        if chars.len() <= first + last {
            return value.to_string();
        }
        let start: String = chars[..first].iter().collect();
        let end: String = chars[chars.len() - last..].iter().collect();
        let middle = mask_char.to_string().repeat(chars.len() - first - last);
        format!("{}{}{}", start, middle, end)
    }

    /// Mask credit card number
    fn mask_credit_card(&self, value: &str) -> String {
        let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() < 4 {
            return "*".repeat(value.len());
        }
        let last_four = &digits[digits.len() - 4..];
        format!("****-****-****-{}", last_four)
    }

    /// Mask SSN
    fn mask_ssn(&self, value: &str) -> String {
        let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() < 4 {
            return "***-**-****".to_string();
        }
        let last_four = &digits[digits.len() - 4..];
        format!("***-**-{}", last_four)
    }

    /// Mask email
    fn mask_email(&self, value: &str) -> String {
        if let Some((local, domain)) = value.split_once('@') {
            let masked_local = if local.len() > 1 {
                format!("{}***", local.chars().next().unwrap())
            } else {
                "***".to_string()
            };
            format!("{}@{}", masked_local, domain)
        } else {
            "***@***.***".to_string()
        }
    }

    /// Mask phone number
    fn mask_phone(&self, value: &str) -> String {
        let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() < 4 {
            return "(***) ***-****".to_string();
        }
        let last_four = &digits[digits.len() - 4..];
        format!("(***) ***-{}", last_four)
    }

    /// Hash value for consistent anonymization
    fn hash_value(&self, value: &str, custom_salt: Option<&str>) -> String {
        let salt = custom_salt.unwrap_or(&self.hash_salt);
        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        hasher.update(value.as_bytes());
        let hash = hasher.finalize();
        format!("HASH_{}", hex::encode(&hash[..8]))
    }

    /// Regex-based masking
    fn regex_mask(&self, value: &str, pattern: &str, replacement: &str) -> OrbitResult<String> {
        let re = regex::Regex::new(pattern)
            .map_err(|e| OrbitError::internal(format!("Invalid regex pattern: {}", e)))?;
        Ok(re.replace_all(value, replacement).to_string())
    }

    /// Generalize date to specified precision
    fn generalize_date(&self, value: &str, precision: DatePrecision) -> OrbitResult<String> {
        // Try to parse various date formats
        let date = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .or_else(|_| chrono::NaiveDate::parse_from_str(value, "%m/%d/%Y"))
            .or_else(|_| chrono::NaiveDate::parse_from_str(value, "%d-%m-%Y"));

        if let Ok(d) = date {
            match precision {
                DatePrecision::Year => Ok(format!("{}-01-01", d.format("%Y"))),
                DatePrecision::Month => Ok(format!("{}-01", d.format("%Y-%m"))),
                DatePrecision::Day => Ok(d.format("%Y-%m-%d").to_string()),
                DatePrecision::Hour => Ok(d.format("%Y-%m-%d").to_string()),
            }
        } else {
            // If can't parse, redact
            Ok("[DATE REDACTED]".to_string())
        }
    }

    /// Mask a row according to policy
    pub async fn mask_row(
        &self,
        table_name: &str,
        row: &HashMap<String, serde_json::Value>,
        context: &MaskingContext,
    ) -> OrbitResult<HashMap<String, serde_json::Value>> {
        let policies = self.policies.read().await;
        let policy = match policies.get(table_name) {
            Some(p) if p.enabled => p,
            _ => return Ok(row.clone()),
        };

        let mut masked_row = HashMap::new();

        for (field_name, value) in row {
            if let Some(config) = policy.field_configs.get(field_name) {
                if let Some(strategy) = config.get_strategy_for_context(context) {
                    // Apply masking
                    if config.preserve_null && value.is_null() {
                        masked_row.insert(field_name.clone(), serde_json::Value::Null);
                    } else {
                        let masked = self.mask_value(value, strategy).await?;
                        masked_row.insert(field_name.clone(), masked);
                    }
                } else {
                    // No masking for this context
                    masked_row.insert(field_name.clone(), value.clone());
                }
            } else {
                // Field not in policy, keep as-is
                masked_row.insert(field_name.clone(), value.clone());
            }
        }

        Ok(masked_row)
    }

    /// Mask multiple rows
    pub async fn mask_rows(
        &self,
        table_name: &str,
        rows: &[HashMap<String, serde_json::Value>],
        context: &MaskingContext,
    ) -> OrbitResult<Vec<HashMap<String, serde_json::Value>>> {
        let mut masked_rows = Vec::with_capacity(rows.len());
        for row in rows {
            masked_rows.push(self.mask_row(table_name, row, context).await?);
        }
        Ok(masked_rows)
    }

    /// Shuffle values within a column (for batch operations)
    pub fn shuffle_column(values: &mut [serde_json::Value]) {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        values.shuffle(&mut rng);
    }
}

impl Default for DataMaskingEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_context(level: AccessLevel) -> MaskingContext {
        MaskingContext::new(level)
    }

    #[tokio::test]
    async fn test_credit_card_masking() {
        let engine = DataMaskingEngine::new();
        let strategy = MaskingStrategy::CreditCard;

        let value = serde_json::json!("4111111111111234");
        let masked = engine.mask_value(&value, &strategy).await.unwrap();

        assert_eq!(masked.as_str().unwrap(), "****-****-****-1234");
    }

    #[tokio::test]
    async fn test_ssn_masking() {
        let engine = DataMaskingEngine::new();
        let strategy = MaskingStrategy::Ssn;

        let value = serde_json::json!("123-45-6789");
        let masked = engine.mask_value(&value, &strategy).await.unwrap();

        assert_eq!(masked.as_str().unwrap(), "***-**-6789");
    }

    #[tokio::test]
    async fn test_email_masking() {
        let engine = DataMaskingEngine::new();
        let strategy = MaskingStrategy::Email;

        let value = serde_json::json!("john.doe@example.com");
        let masked = engine.mask_value(&value, &strategy).await.unwrap();

        assert_eq!(masked.as_str().unwrap(), "j***@example.com");
    }

    #[tokio::test]
    async fn test_phone_masking() {
        let engine = DataMaskingEngine::new();
        let strategy = MaskingStrategy::Phone;

        let value = serde_json::json!("(555) 123-4567");
        let masked = engine.mask_value(&value, &strategy).await.unwrap();

        assert_eq!(masked.as_str().unwrap(), "(***) ***-4567");
    }

    #[tokio::test]
    async fn test_partial_masking() {
        let engine = DataMaskingEngine::new();

        // Show last 4
        let strategy = MaskingStrategy::ShowLast {
            count: 4,
            mask_char: '*',
        };
        let value = serde_json::json!("1234567890");
        let masked = engine.mask_value(&value, &strategy).await.unwrap();
        assert_eq!(masked.as_str().unwrap(), "******7890");

        // Show first 3
        let strategy = MaskingStrategy::ShowFirst {
            count: 3,
            mask_char: 'X',
        };
        let masked = engine.mask_value(&value, &strategy).await.unwrap();
        assert_eq!(masked.as_str().unwrap(), "123XXXXXXX");

        // Show first and last
        let strategy = MaskingStrategy::ShowFirstAndLast {
            first: 2,
            last: 2,
            mask_char: '*',
        };
        let masked = engine.mask_value(&value, &strategy).await.unwrap();
        assert_eq!(masked.as_str().unwrap(), "12******90");
    }

    #[tokio::test]
    async fn test_hash_masking() {
        let engine = DataMaskingEngine::with_salt("test_salt");
        let strategy = MaskingStrategy::Hash { salt: None };

        let value = serde_json::json!("sensitive_data");
        let masked1 = engine.mask_value(&value, &strategy).await.unwrap();
        let masked2 = engine.mask_value(&value, &strategy).await.unwrap();

        // Hash should be consistent
        assert_eq!(masked1, masked2);
        assert!(masked1.as_str().unwrap().starts_with("HASH_"));
    }

    #[tokio::test]
    async fn test_numeric_range_masking() {
        let engine = DataMaskingEngine::new();
        let strategy = MaskingStrategy::NumericRange {
            bucket_size: 10000.0,
        };

        let value = serde_json::json!(45678);
        let masked = engine.mask_value(&value, &strategy).await.unwrap();

        assert_eq!(masked.as_str().unwrap(), "40000-50000");
    }

    #[tokio::test]
    async fn test_row_masking() {
        let engine = DataMaskingEngine::new();

        let policy = MaskingPolicy::new("employees")
            .mask_field("ssn", MaskingStrategy::Ssn)
            .mask_field(
                "salary",
                MaskingStrategy::NumericRange {
                    bucket_size: 10000.0,
                },
            )
            .mask_field("email", MaskingStrategy::Email);

        engine.register_policy(policy).await.unwrap();

        let row: HashMap<String, serde_json::Value> = [
            ("id".to_string(), serde_json::json!(1)),
            ("name".to_string(), serde_json::json!("John Doe")),
            ("ssn".to_string(), serde_json::json!("123-45-6789")),
            ("salary".to_string(), serde_json::json!(75000)),
            ("email".to_string(), serde_json::json!("john@company.com")),
        ]
        .into_iter()
        .collect();

        let context = create_test_context(AccessLevel::Standard);
        let masked = engine.mask_row("employees", &row, &context).await.unwrap();

        // id and name should be unchanged
        assert_eq!(masked.get("id"), row.get("id"));
        assert_eq!(masked.get("name"), row.get("name"));

        // ssn, salary, email should be masked
        assert_eq!(masked.get("ssn").unwrap().as_str().unwrap(), "***-**-6789");
        assert_eq!(
            masked.get("salary").unwrap().as_str().unwrap(),
            "70000-80000"
        );
        assert_eq!(
            masked.get("email").unwrap().as_str().unwrap(),
            "j***@company.com"
        );
    }

    #[tokio::test]
    async fn test_access_level_override() {
        let engine = DataMaskingEngine::new();

        let config = FieldMaskingConfig::new("ssn", MaskingStrategy::Ssn)
            .with_access_override(AccessLevel::Full, None); // No masking for full access

        let policy = MaskingPolicy::new("users").mask_field_with_config(config);
        engine.register_policy(policy).await.unwrap();

        let row: HashMap<String, serde_json::Value> =
            [("ssn".to_string(), serde_json::json!("123-45-6789"))]
                .into_iter()
                .collect();

        // Standard access - should be masked
        let standard_ctx = create_test_context(AccessLevel::Standard);
        let masked = engine.mask_row("users", &row, &standard_ctx).await.unwrap();
        assert_eq!(masked.get("ssn").unwrap().as_str().unwrap(), "***-**-6789");

        // Full access - should not be masked
        let full_ctx = create_test_context(AccessLevel::Full);
        let unmasked = engine.mask_row("users", &row, &full_ctx).await.unwrap();
        assert_eq!(
            unmasked.get("ssn").unwrap().as_str().unwrap(),
            "123-45-6789"
        );
    }

    #[tokio::test]
    async fn test_data_owner_bypass() {
        let engine = DataMaskingEngine::new();

        let policy = MaskingPolicy::new("data").mask_field(
            "secret",
            MaskingStrategy::Redact {
                replacement: Some("[HIDDEN]".to_string()),
            },
        );
        engine.register_policy(policy).await.unwrap();

        let row: HashMap<String, serde_json::Value> =
            [("secret".to_string(), serde_json::json!("top_secret_value"))]
                .into_iter()
                .collect();

        // Regular user - should be redacted
        let regular_ctx = create_test_context(AccessLevel::Standard);
        let masked = engine.mask_row("data", &row, &regular_ctx).await.unwrap();
        assert_eq!(masked.get("secret").unwrap().as_str().unwrap(), "[HIDDEN]");

        // Data owner - should bypass masking
        let owner_ctx = create_test_context(AccessLevel::Standard).as_data_owner();
        let unmasked = engine.mask_row("data", &row, &owner_ctx).await.unwrap();
        assert_eq!(
            unmasked.get("secret").unwrap().as_str().unwrap(),
            "top_secret_value"
        );
    }

    #[tokio::test]
    async fn test_tokenization() {
        let engine = DataMaskingEngine::new();
        let strategy = MaskingStrategy::Tokenize {
            token_prefix: "TKN".to_string(),
        };

        let value = serde_json::json!("sensitive_value");

        // Tokenize
        let token1 = engine.mask_value(&value, &strategy).await.unwrap();
        let token2 = engine.mask_value(&value, &strategy).await.unwrap();

        // Same value should produce same token
        assert_eq!(token1, token2);
        assert!(token1.as_str().unwrap().starts_with("TKN_"));

        // Can detokenize
        let original = engine
            .token_store()
            .detokenize(token1.as_str().unwrap())
            .await;
        assert_eq!(original.unwrap(), "sensitive_value");
    }

    #[tokio::test]
    async fn test_date_generalization() {
        let engine = DataMaskingEngine::new();

        // Year precision
        let strategy = MaskingStrategy::DateGeneralize {
            precision: DatePrecision::Year,
        };
        let value = serde_json::json!("2024-06-15");
        let masked = engine.mask_value(&value, &strategy).await.unwrap();
        assert_eq!(masked.as_str().unwrap(), "2024-01-01");

        // Month precision
        let strategy = MaskingStrategy::DateGeneralize {
            precision: DatePrecision::Month,
        };
        let masked = engine.mask_value(&value, &strategy).await.unwrap();
        assert_eq!(masked.as_str().unwrap(), "2024-06-01");
    }

    #[tokio::test]
    async fn test_role_override() {
        let engine = DataMaskingEngine::new();

        let config =
            FieldMaskingConfig::new("salary", MaskingStrategy::Redact { replacement: None })
                .with_role_override("hr_admin", None); // HR admins can see salary

        let policy = MaskingPolicy::new("employees").mask_field_with_config(config);
        engine.register_policy(policy).await.unwrap();

        let row: HashMap<String, serde_json::Value> =
            [("salary".to_string(), serde_json::json!(100000))]
                .into_iter()
                .collect();

        // Regular user - should be redacted
        let regular_ctx = create_test_context(AccessLevel::Standard);
        let masked = engine
            .mask_row("employees", &row, &regular_ctx)
            .await
            .unwrap();
        assert_eq!(
            masked.get("salary").unwrap().as_str().unwrap(),
            "[REDACTED]"
        );

        // HR admin - should not be masked
        let hr_ctx =
            create_test_context(AccessLevel::Standard).with_roles(vec!["hr_admin".to_string()]);
        let unmasked = engine.mask_row("employees", &row, &hr_ctx).await.unwrap();
        assert_eq!(unmasked.get("salary").unwrap().as_i64().unwrap(), 100000);
    }
}
