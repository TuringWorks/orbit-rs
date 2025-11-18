//! Extension traits for adding common functionality
//!
//! This module provides extension traits that add useful methods to
//! standard types, reducing code duplication.

use crate::error::{OrbitError, OrbitResult};
use std::collections::HashMap;
use std::hash::Hash;

/// Extension trait for Result types
pub trait ResultExt<T, E> {
    /// Convert any error type to OrbitError with context
    fn or_internal_error(self, context: &str) -> OrbitResult<T>;

    /// Log error and continue with default value
    fn or_default_with_log(self, message: &str) -> T
    where
        T: Default;

    /// Tap into Ok value without consuming
    fn tap_ok<F>(self, f: F) -> Self
    where
        F: FnOnce(&T);

    /// Tap into Err value without consuming
    fn tap_err<F>(self, f: F) -> Self
    where
        F: FnOnce(&E);
}

impl<T, E: std::error::Error> ResultExt<T, E> for Result<T, E> {
    fn or_internal_error(self, context: &str) -> OrbitResult<T> {
        self.map_err(|e| OrbitError::internal(format!("{}: {}", context, e)))
    }

    fn or_default_with_log(self, message: &str) -> T
    where
        T: Default,
    {
        match self {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("{}: {}", message, e);
                T::default()
            }
        }
    }

    fn tap_ok<F>(self, f: F) -> Self
    where
        F: FnOnce(&T),
    {
        if let Ok(ref v) = self {
            f(v);
        }
        self
    }

    fn tap_err<F>(self, f: F) -> Self
    where
        F: FnOnce(&E),
    {
        if let Err(ref e) = self {
            f(e);
        }
        self
    }
}

/// Extension trait for Option types
pub trait OptionExt<T> {
    /// Convert None to OrbitError
    fn ok_or_internal(self, message: &str) -> OrbitResult<T>;

    /// Tap into Some value without consuming
    fn tap_some<F>(self, f: F) -> Self
    where
        F: FnOnce(&T);

    /// Tap into None without consuming
    fn tap_none<F>(self, f: F) -> Self
    where
        F: FnOnce();
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_or_internal(self, message: &str) -> OrbitResult<T> {
        self.ok_or_else(|| OrbitError::internal(message))
    }

    fn tap_some<F>(self, f: F) -> Self
    where
        F: FnOnce(&T),
    {
        if let Some(ref v) = self {
            f(v);
        }
        self
    }

    fn tap_none<F>(self, f: F) -> Self
    where
        F: FnOnce(),
    {
        if self.is_none() {
            f();
        }
        self
    }
}

/// Extension trait for HashMap
pub trait HashMapExt<K, V> {
    /// Get value or insert default and return reference
    fn get_or_insert_default(&mut self, key: K) -> &mut V
    where
        V: Default,
        K: Clone;

    /// Get value or compute and insert
    fn get_or_insert_with<F>(&mut self, key: K, f: F) -> &mut V
    where
        F: FnOnce() -> V,
        K: Clone;

    /// Try to get value or return error
    fn try_get(&self, key: &K) -> OrbitResult<&V>;

    /// Try to remove value or return error
    fn try_remove(&mut self, key: &K) -> OrbitResult<V>
    where
        K: Clone;
}

impl<K, V> HashMapExt<K, V> for HashMap<K, V>
where
    K: Eq + Hash + std::fmt::Display,
{
    fn get_or_insert_default(&mut self, key: K) -> &mut V
    where
        V: Default,
        K: Clone,
    {
        self.entry(key).or_insert_with(V::default)
    }

    fn get_or_insert_with<F>(&mut self, key: K, f: F) -> &mut V
    where
        F: FnOnce() -> V,
        K: Clone,
    {
        self.entry(key).or_insert_with(f)
    }

    fn try_get(&self, key: &K) -> OrbitResult<&V> {
        self.get(key)
            .ok_or_else(|| OrbitError::internal(format!("Key not found: {}", key)))
    }

    fn try_remove(&mut self, key: &K) -> OrbitResult<V>
    where
        K: Clone,
    {
        self.remove(key)
            .ok_or_else(|| OrbitError::internal(format!("Key not found: {}", key)))
    }
}

/// Extension trait for Vec
pub trait VecExt<T> {
    /// Partition vector into two based on predicate, consuming self
    fn partition_map<F, L, R>(self, f: F) -> (Vec<L>, Vec<R>)
    where
        F: FnMut(T) -> Result<L, R>;

    /// Try to get element or return error
    fn try_get(&self, index: usize) -> OrbitResult<&T>;

    /// Chunk into groups of size n
    fn chunks_exact_vec(&self, chunk_size: usize) -> Vec<&[T]>;
}

impl<T> VecExt<T> for Vec<T> {
    fn partition_map<F, L, R>(self, mut f: F) -> (Vec<L>, Vec<R>)
    where
        F: FnMut(T) -> Result<L, R>,
    {
        let mut left = Vec::new();
        let mut right = Vec::new();

        for item in self {
            match f(item) {
                Ok(l) => left.push(l),
                Err(r) => right.push(r),
            }
        }

        (left, right)
    }

    fn try_get(&self, index: usize) -> OrbitResult<&T> {
        self.get(index)
            .ok_or_else(|| OrbitError::internal(format!("Index out of bounds: {}", index)))
    }

    fn chunks_exact_vec(&self, chunk_size: usize) -> Vec<&[T]> {
        self.chunks(chunk_size).collect()
    }
}

/// Extension trait for iterators
pub trait IteratorExt: Iterator {
    /// Try to find first item matching predicate
    fn try_find_orbit<F>(&mut self, f: F) -> OrbitResult<Option<Self::Item>>
    where
        F: FnMut(&Self::Item) -> OrbitResult<bool>,
        Self: Sized;
}

impl<I: Iterator> IteratorExt for I {
    fn try_find_orbit<F>(&mut self, mut f: F) -> OrbitResult<Option<Self::Item>>
    where
        F: FnMut(&Self::Item) -> OrbitResult<bool>,
        Self: Sized,
    {
        for item in self {
            if f(&item)? {
                return Ok(Some(item));
            }
        }
        Ok(None)
    }
}

/// Extension trait for String
pub trait StringExt {
    /// Truncate string to max length with ellipsis
    fn truncate_with_ellipsis(&self, max_len: usize) -> String;

    /// Check if string is a valid identifier (alphanumeric + underscore)
    fn is_valid_identifier(&self) -> bool;

    /// Convert to snake_case
    fn to_snake_case(&self) -> String;

    /// Convert to CamelCase
    fn to_camel_case(&self) -> String;
}

impl StringExt for String {
    fn truncate_with_ellipsis(&self, max_len: usize) -> String {
        if self.len() <= max_len {
            self.clone()
        } else {
            format!("{}...", &self[..max_len.saturating_sub(3)])
        }
    }

    fn is_valid_identifier(&self) -> bool {
        !self.is_empty()
            && self
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            && !self.chars().next().unwrap().is_numeric()
    }

    fn to_snake_case(&self) -> String {
        let mut result = String::new();
        for (i, ch) in self.chars().enumerate() {
            if ch.is_uppercase() {
                if i > 0 {
                    result.push('_');
                }
                result.push(ch.to_lowercase().next().unwrap());
            } else {
                result.push(ch);
            }
        }
        result
    }

    fn to_camel_case(&self) -> String {
        let mut result = String::new();
        let mut capitalize_next = true;

        for ch in self.chars() {
            if ch == '_' || ch == '-' {
                capitalize_next = true;
            } else if capitalize_next {
                result.push(ch.to_uppercase().next().unwrap());
                capitalize_next = false;
            } else {
                result.push(ch);
            }
        }
        result
    }
}

impl StringExt for str {
    fn truncate_with_ellipsis(&self, max_len: usize) -> String {
        if self.len() <= max_len {
            self.to_string()
        } else {
            format!("{}...", &self[..max_len.saturating_sub(3)])
        }
    }

    fn is_valid_identifier(&self) -> bool {
        !self.is_empty()
            && self
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            && !self.chars().next().unwrap().is_numeric()
    }

    fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    fn to_camel_case(&self) -> String {
        self.to_string().to_camel_case()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_ext() {
        let ok_result: Result<i32, std::io::Error> = Ok(42);
        let orbit_result = ok_result.or_internal_error("test context");
        assert!(orbit_result.is_ok());

        let err_result: Result<i32, std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::Other, "test"));
        let orbit_result = err_result.or_internal_error("test context");
        assert!(orbit_result.is_err());
    }

    #[test]
    fn test_option_ext() {
        let some_val: Option<i32> = Some(42);
        assert!(some_val.ok_or_internal("not found").is_ok());

        let none_val: Option<i32> = None;
        assert!(none_val.ok_or_internal("not found").is_err());
    }

    #[test]
    fn test_hashmap_ext() {
        let mut map = HashMap::new();
        map.insert("key1", 1);

        assert_eq!(*map.try_get(&"key1").unwrap(), 1);
        assert!(map.try_get(&"key2").is_err());
    }

    #[test]
    fn test_vec_ext() {
        let vec = vec![1, 2, 3, 4, 5];
        assert_eq!(*vec.try_get(0).unwrap(), 1);
        assert!(vec.try_get(10).is_err());
    }

    #[test]
    fn test_string_ext() {
        assert_eq!("Hello World".truncate_with_ellipsis(8), "Hello...");
        assert_eq!("Short".truncate_with_ellipsis(10), "Short");

        assert!("valid_name".is_valid_identifier());
        assert!(!"123invalid".is_valid_identifier());
        assert!("with-dash".is_valid_identifier());

        assert_eq!("HelloWorld".to_snake_case(), "hello_world");
        assert_eq!("hello_world".to_camel_case(), "HelloWorld");
    }
}
