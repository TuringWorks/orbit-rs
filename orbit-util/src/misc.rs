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
    use std::collections::HashSet;

    #[test]
    fn test_random_string_default_length() {
        let s = RngUtils::random_string();
        assert_eq!(s.len(), 16);
        assert!(s.chars().all(|c| c.is_alphanumeric()));

        // Test that multiple calls produce different results
        let s2 = RngUtils::random_string();
        assert_ne!(s, s2, "Random strings should be different");
    }

    #[test]
    fn test_random_string_custom_length() {
        let s = RngUtils::random_string_with_length(10);
        assert_eq!(s.len(), 10);
        assert!(s.chars().all(|c| c.is_alphanumeric()));

        // Test various lengths
        let lengths = [1, 5, 32, 100, 256];
        for &len in &lengths {
            let s = RngUtils::random_string_with_length(len);
            assert_eq!(s.len(), len, "String length should be {}", len);
            assert!(s.chars().all(|c| c.is_alphanumeric()));
        }
    }

    #[test]
    fn test_random_string_zero_length() {
        let s = RngUtils::random_string_with_length(0);
        assert_eq!(s.len(), 0);
        assert_eq!(s, "");
    }

    #[test]
    fn test_random_string_uniqueness() {
        // Generate multiple strings and ensure they're mostly unique
        let mut strings = HashSet::new();
        for _ in 0..100 {
            let s = RngUtils::random_string_with_length(8);
            strings.insert(s);
        }

        // With 8-character alphanumeric strings, we should have high uniqueness
        // Allow some small chance of collision but expect mostly unique strings
        assert!(
            strings.len() > 95,
            "Expected high uniqueness, got {} unique strings out of 100",
            strings.len()
        );
    }

    #[test]
    fn test_random_string_character_distribution() {
        let s = RngUtils::random_string_with_length(1000);

        let mut has_digit = false;
        let mut has_lowercase = false;
        let mut has_uppercase = false;

        for c in s.chars() {
            if c.is_ascii_digit() {
                has_digit = true;
            } else if c.is_ascii_lowercase() {
                has_lowercase = true;
            } else if c.is_ascii_uppercase() {
                has_uppercase = true;
            }
        }

        // With 1000 characters, we should see all types
        assert!(has_digit, "Should contain at least one digit");
        assert!(
            has_lowercase,
            "Should contain at least one lowercase letter"
        );
        assert!(
            has_uppercase,
            "Should contain at least one uppercase letter"
        );
    }

    #[test]
    fn test_random_int_range() {
        let n = RngUtils::random_int(10, 20);
        assert!((10..=20).contains(&n));

        // Test multiple values to ensure they're within range
        for _ in 0..100 {
            let n = RngUtils::random_int(1, 10);
            assert!(
                (1..=10).contains(&n),
                "Random int {} not in range 1..=10",
                n
            );
        }
    }

    #[test]
    fn test_random_int_edge_cases() {
        // Test same min and max
        let n = RngUtils::random_int(5, 5);
        assert_eq!(n, 5);

        // Test negative ranges
        let n = RngUtils::random_int(-10, -5);
        assert!((-10..=-5).contains(&n));

        // Test crossing zero
        let n = RngUtils::random_int(-5, 5);
        assert!((-5..=5).contains(&n));

        // Test large values
        let n = RngUtils::random_int(i32::MAX - 10, i32::MAX);
        assert!((i32::MAX - 10..=i32::MAX).contains(&n));
    }

    #[test]
    fn test_random_int_distribution() {
        // Test that we get reasonable distribution across range
        let mut counts = [0; 10];

        for _ in 0..1000 {
            let n = RngUtils::random_int(0, 9);
            counts[n as usize] += 1;
        }

        // Each number should appear at least a few times
        // With uniform distribution and 1000 samples, each should get ~100
        // Allow for some variance but ensure no number is completely missing
        for (i, &count) in counts.iter().enumerate() {
            assert!(
                count > 50,
                "Number {} appeared only {} times, expected more",
                i,
                count
            );
        }
    }

    #[test]
    fn test_random_u64() {
        let n1 = RngUtils::random_u64();
        let n2 = RngUtils::random_u64();

        // Very unlikely to be the same
        assert_ne!(n1, n2, "Two random u64s should be different");

        // Test multiple values to ensure they're generated successfully
        for _ in 0..10 {
            let n = RngUtils::random_u64();
            // Just verify the function returns a valid u64 (any value is valid)
            let _ = n; // Use the value to avoid unused variable warning
        }
    }

    #[test]
    fn test_random_u64_distribution() {
        // Test that we get good distribution across the range
        let mut values = Vec::new();
        for _ in 0..100 {
            values.push(RngUtils::random_u64());
        }

        // All values should be unique (very high probability)
        let unique_count = values.iter().collect::<HashSet<_>>().len();
        assert!(
            unique_count > 95,
            "Expected high uniqueness for u64s, got {}",
            unique_count
        );

        // Should have some spread across the range
        let min_val = values.iter().min().unwrap();
        let max_val = values.iter().max().unwrap();

        // With 100 random u64s, the range should be quite large
        // This is probabilistic but very likely to pass
        assert!(max_val > min_val, "Max should be greater than min");
    }

    #[test]
    fn test_alphanumeric_characters() {
        // Verify that the character set includes what we expect
        let s = RngUtils::random_string_with_length(10000);

        let mut char_set = HashSet::new();
        for c in s.chars() {
            char_set.insert(c);
            // Ensure each character is alphanumeric
            assert!(
                c.is_alphanumeric(),
                "Character '{}' should be alphanumeric",
                c
            );
            assert!(c.is_ascii(), "Character '{}' should be ASCII", c);
        }

        // With 10000 characters, we should see a good variety
        // Alphanumeric includes [a-zA-Z0-9] = 62 characters
        assert!(
            char_set.len() > 50,
            "Should see most alphanumeric characters, got {}",
            char_set.len()
        );
    }

    #[test]
    fn test_concurrent_safety() {
        // Test that random generation works in concurrent environment
        use std::sync::Arc;
        use std::sync::Mutex;
        use std::thread;

        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();

        for _ in 0..10 {
            let results_clone = Arc::clone(&results);
            let handle = thread::spawn(move || {
                let s = RngUtils::random_string_with_length(8);
                let n = RngUtils::random_int(1, 100);
                let u = RngUtils::random_u64();

                results_clone.lock().unwrap().push((s, n, u));
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let results = results.lock().unwrap();
        assert_eq!(results.len(), 10);

        // Verify all results are valid
        for (s, n, _u) in results.iter() {
            assert_eq!(s.len(), 8);
            assert!((1..=100).contains(n));
        }

        // Verify uniqueness (high probability)
        let strings: HashSet<_> = results.iter().map(|(s, _, _)| s).collect();
        assert!(strings.len() > 8, "Most strings should be unique");
    }
}
