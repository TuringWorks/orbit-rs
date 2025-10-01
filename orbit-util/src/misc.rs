use rand::{distributions::Alphanumeric, thread_rng, Rng};

/// Utility functions for random generation, equivalent to Kotlin's RNGUtils
pub struct RngUtils;

impl RngUtils {
    /// Generate a random string of default length (16 characters)
    pub fn random_string() -> String {
        Self::random_string_with_length(16)
    }

    /// Generate a random string of specified length using alphanumeric characters
    pub fn random_string_with_length(length: usize) -> String {
        thread_rng()
            .sample_iter(&Alphanumeric)
            .take(length)
            .map(char::from)
            .collect()
    }

    /// Generate a random integer within a range
    pub fn random_int(min: i32, max: i32) -> i32 {
        thread_rng().gen_range(min..=max)
    }

    /// Generate a random u64
    pub fn random_u64() -> u64 {
        thread_rng().gen()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_string_default_length() {
        let s = RngUtils::random_string();
        assert_eq!(s.len(), 16);
        assert!(s.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_random_string_custom_length() {
        let s = RngUtils::random_string_with_length(10);
        assert_eq!(s.len(), 10);
        assert!(s.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_random_int_range() {
        let n = RngUtils::random_int(10, 20);
        assert!(n >= 10 && n <= 20);
    }
}
