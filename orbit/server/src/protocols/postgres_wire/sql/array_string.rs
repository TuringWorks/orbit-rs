// Array and String Utility Functions Module
//
// Implements array and string utility functions for OrbitQL:
// - array::group() - Group elements into nested arrays
// - string::join() - Join array of strings with delimiter

use serde_json::{json, Value};

use crate::protocols::{ProtocolError, ProtocolResult};

/// Group array elements into nested arrays of specified size
///
/// Usage: array::group([1, 2, 3, 4, 5, 6], 2)
/// Returns: [[1, 2], [3, 4], [5, 6]]
pub fn array_group(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 2 {
        return Err(ProtocolError::PostgresError(
            "array::group() expects 2 arguments (array, size)".to_string(),
        ));
    }

    let array = args[0].as_array().ok_or_else(|| {
        ProtocolError::PostgresError("First argument must be an array".to_string())
    })?;

    let size = args[1]
        .as_u64()
        .ok_or_else(|| ProtocolError::PostgresError("Size must be a positive number".to_string()))?
        as usize;

    if size == 0 {
        return Err(ProtocolError::PostgresError(
            "Size must be greater than 0".to_string(),
        ));
    }

    let mut result: Vec<Value> = Vec::new();
    let mut current_group: Vec<Value> = Vec::new();

    for (i, item) in array.iter().enumerate() {
        current_group.push(item.clone());

        if (i + 1) % size == 0 || i == array.len() - 1 {
            result.push(json!(current_group));
            current_group = Vec::new();
        }
    }

    Ok(json!(result))
}

/// Join array of strings with a delimiter
///
/// Usage: string::join(['Hello', 'world', 'from', 'OrbitRS'], ' ')
/// Returns: 'Hello world from OrbitRS'
pub fn string_join(args: &[Value]) -> ProtocolResult<Value> {
    if args.is_empty() || args.len() > 2 {
        return Err(ProtocolError::PostgresError(
            "string::join() expects 1-2 arguments (array, [delimiter])".to_string(),
        ));
    }

    let array = args[0].as_array().ok_or_else(|| {
        ProtocolError::PostgresError("First argument must be an array".to_string())
    })?;

    let delimiter = if args.len() > 1 {
        args[1]
            .as_str()
            .ok_or_else(|| ProtocolError::PostgresError("Delimiter must be a string".to_string()))?
    } else {
        ""
    };

    let strings: Result<Vec<String>, _> = array
        .iter()
        .map(|v| {
            v.as_str().map(|s| s.to_string()).ok_or_else(|| {
                ProtocolError::PostgresError("All array elements must be strings".to_string())
            })
        })
        .collect();

    let strings = strings?;
    let joined = strings.join(delimiter);

    Ok(json!(joined))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_array_group() {
        let array = json!([1, 2, 3, 4, 5, 6]);
        let result = array_group(&[array, json!(2)]).unwrap();
        assert_eq!(result, json!([[1, 2], [3, 4], [5, 6]]));
    }

    #[test]
    fn test_array_group_uneven() {
        let array = json!([1, 2, 3, 4, 5]);
        let result = array_group(&[array, json!(2)]).unwrap();
        assert_eq!(result, json!([[1, 2], [3, 4], [5]]));
    }

    #[test]
    fn test_array_group_size_one() {
        let array = json!([1, 2, 3]);
        let result = array_group(&[array, json!(1)]).unwrap();
        assert_eq!(result, json!([[1], [2], [3]]));
    }

    #[test]
    fn test_array_group_size_larger_than_array() {
        let array = json!([1, 2, 3]);
        let result = array_group(&[array, json!(5)]).unwrap();
        assert_eq!(result, json!([[1, 2, 3]]));
    }

    #[test]
    fn test_string_join() {
        let array = json!(["Hello", "world", "from", "OrbitRS"]);
        let result = string_join(&[array, json!(" ")]).unwrap();
        assert_eq!(result, json!("Hello world from OrbitRS"));
    }

    #[test]
    fn test_string_join_no_delimiter() {
        let array = json!(["Hello", "world"]);
        let result = string_join(&[array]).unwrap();
        assert_eq!(result, json!("Helloworld"));
    }

    #[test]
    fn test_string_join_custom_delimiter() {
        let array = json!(["apple", "banana", "cherry"]);
        let result = string_join(&[array, json!(", ")]).unwrap();
        assert_eq!(result, json!("apple, banana, cherry"));
    }

    #[test]
    fn test_string_join_empty_array() {
        let array = json!([]);
        let result = string_join(&[array, json!(", ")]).unwrap();
        assert_eq!(result, json!(""));
    }

    #[test]
    fn test_array_group_error_handling() {
        // Wrong number of arguments
        assert!(array_group(&[]).is_err());
        assert!(array_group(&[json!([1, 2, 3])]).is_err());

        // Non-array first argument
        assert!(array_group(&[json!(123), json!(2)]).is_err());

        // Non-number size
        assert!(array_group(&[json!([1, 2, 3]), json!("2")]).is_err());

        // Zero size
        assert!(array_group(&[json!([1, 2, 3]), json!(0)]).is_err());
    }

    #[test]
    fn test_string_join_error_handling() {
        // Wrong number of arguments
        assert!(string_join(&[]).is_err());
        assert!(string_join(&[json!([1]), json!(","), json!("extra")]).is_err());

        // Non-array first argument
        assert!(string_join(&[json!(123)]).is_err());

        // Non-string elements
        assert!(string_join(&[json!([1, 2, 3]), json!(",")]).is_err());

        // Non-string delimiter
        assert!(string_join(&[json!(["a", "b"]), json!(123)]).is_err());
    }
}
