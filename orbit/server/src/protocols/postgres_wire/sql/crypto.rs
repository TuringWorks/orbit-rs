// Crypto Functions Module
//
// Implements cryptographic hash functions for OrbitQL:
// - crypto::argon2::compare() - Verify Argon2 password hash
// - crypto::argon2::generate() - Generate Argon2 password hash
// - crypto::bcrypt::compare() - Verify bcrypt password hash
// - crypto::bcrypt::generate() - Generate bcrypt password hash
// - crypto::pbkdf2::compare() - Verify PBKDF2 password hash
// - crypto::pbkdf2::generate() - Generate PBKDF2 password hash

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use pbkdf2::{password_hash::SaltString as Pbkdf2Salt, Pbkdf2};
use serde_json::{json, Value};

use crate::protocols::{ProtocolError, ProtocolResult};

/// Generate Argon2 password hash
///
/// Usage: crypto::argon2::generate('my_password')
/// Returns: Argon2id hash string (PHC format)
pub fn argon2_generate(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 1 {
        return Err(ProtocolError::PostgresError(
            "crypto::argon2::generate() expects 1 argument (password)".to_string(),
        ));
    }

    let password = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("Password must be a string".to_string()))?;

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| ProtocolError::PostgresError(format!("Argon2 hash failed: {}", e)))?;

    Ok(json!(hash.to_string()))
}

/// Verify Argon2 password hash
///
/// Usage: crypto::argon2::compare('my_password', '$argon2id$...')
/// Returns: true if password matches, false otherwise
pub fn argon2_compare(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 2 {
        return Err(ProtocolError::PostgresError(
            "crypto::argon2::compare() expects 2 arguments (password, hash)".to_string(),
        ));
    }

    let password = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("Password must be a string".to_string()))?;
    let hash_str = args[1]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("Hash must be a string".to_string()))?;

    let parsed_hash = PasswordHash::new(hash_str)
        .map_err(|e| ProtocolError::PostgresError(format!("Invalid Argon2 hash format: {}", e)))?;

    let argon2 = Argon2::default();
    let is_valid = argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok();

    Ok(json!(is_valid))
}

/// Generate bcrypt password hash
///
/// Usage: crypto::bcrypt::generate('my_password')
/// Returns: bcrypt hash string
pub fn bcrypt_generate(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 1 {
        return Err(ProtocolError::PostgresError(
            "crypto::bcrypt::generate() expects 1 argument (password)".to_string(),
        ));
    }

    let password = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("Password must be a string".to_string()))?;

    let hash_str = hash(password, DEFAULT_COST)
        .map_err(|e| ProtocolError::PostgresError(format!("Bcrypt hash failed: {}", e)))?;

    Ok(json!(hash_str))
}

/// Verify bcrypt password hash
///
/// Usage: crypto::bcrypt::compare('my_password', '$2b$12$...')
/// Returns: true if password matches, false otherwise
pub fn bcrypt_compare(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 2 {
        return Err(ProtocolError::PostgresError(
            "crypto::bcrypt::compare() expects 2 arguments (password, hash)".to_string(),
        ));
    }

    let password = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("Password must be a string".to_string()))?;
    let hash_str = args[1]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("Hash must be a string".to_string()))?;

    let is_valid = verify(password, hash_str)
        .map_err(|e| ProtocolError::PostgresError(format!("Bcrypt verify failed: {}", e)))?;

    Ok(json!(is_valid))
}

/// Generate PBKDF2 password hash
///
/// Usage: crypto::pbkdf2::generate('my_password')
/// Returns: PBKDF2-SHA256 hash string (PHC format)
pub fn pbkdf2_generate(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 1 {
        return Err(ProtocolError::PostgresError(
            "crypto::pbkdf2::generate() expects 1 argument (password)".to_string(),
        ));
    }

    let password = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("Password must be a string".to_string()))?;

    let salt = Pbkdf2Salt::generate(&mut OsRng);
    let hash = Pbkdf2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| ProtocolError::PostgresError(format!("PBKDF2 hash failed: {}", e)))?;

    Ok(json!(hash.to_string()))
}

/// Verify PBKDF2 password hash
///
/// Usage: crypto::pbkdf2::compare('my_password', '$pbkdf2-sha256$...')
/// Returns: true if password matches, false otherwise
pub fn pbkdf2_compare(args: &[Value]) -> ProtocolResult<Value> {
    if args.len() != 2 {
        return Err(ProtocolError::PostgresError(
            "crypto::pbkdf2::compare() expects 2 arguments (password, hash)".to_string(),
        ));
    }

    let password = args[0]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("Password must be a string".to_string()))?;
    let hash_str = args[1]
        .as_str()
        .ok_or_else(|| ProtocolError::PostgresError("Hash must be a string".to_string()))?;

    let parsed_hash = PasswordHash::new(hash_str)
        .map_err(|e| ProtocolError::PostgresError(format!("Invalid PBKDF2 hash format: {}", e)))?;

    let is_valid = Pbkdf2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok();

    Ok(json!(is_valid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argon2_generate_and_compare() {
        let password = json!("my_secret_password");
        let hash = argon2_generate(std::slice::from_ref(&password)).unwrap();
        let hash_str = hash.as_str().unwrap();

        // Should verify successfully
        assert_eq!(
            argon2_compare(&[password.clone(), json!(hash_str)]).unwrap(),
            json!(true)
        );

        // Wrong password should fail
        assert_eq!(
            argon2_compare(&[json!("wrong_password"), json!(hash_str)]).unwrap(),
            json!(false)
        );
    }

    #[test]
    fn test_bcrypt_generate_and_compare() {
        let password = json!("my_secret_password");
        let hash = bcrypt_generate(std::slice::from_ref(&password)).unwrap();
        let hash_str = hash.as_str().unwrap();

        // Should verify successfully
        assert_eq!(
            bcrypt_compare(&[password.clone(), json!(hash_str)]).unwrap(),
            json!(true)
        );

        // Wrong password should fail
        assert_eq!(
            bcrypt_compare(&[json!("wrong_password"), json!(hash_str)]).unwrap(),
            json!(false)
        );
    }

    #[test]
    fn test_pbkdf2_generate_and_compare() {
        let password = json!("my_secret_password");
        let hash = pbkdf2_generate(std::slice::from_ref(&password)).unwrap();
        let hash_str = hash.as_str().unwrap();

        // Should verify successfully
        assert_eq!(
            pbkdf2_compare(&[password.clone(), json!(hash_str)]).unwrap(),
            json!(true)
        );

        // Wrong password should fail
        assert_eq!(
            pbkdf2_compare(&[json!("wrong_password"), json!(hash_str)]).unwrap(),
            json!(false)
        );
    }

    #[test]
    fn test_argon2_error_handling() {
        // Wrong number of arguments
        assert!(argon2_generate(&[]).is_err());
        assert!(argon2_compare(&[json!("password")]).is_err());

        // Non-string arguments
        assert!(argon2_generate(&[json!(123)]).is_err());
        assert!(argon2_compare(&[json!(123), json!("hash")]).is_err());

        // Invalid hash format
        assert!(argon2_compare(&[json!("password"), json!("invalid_hash")]).is_err());
    }
}
