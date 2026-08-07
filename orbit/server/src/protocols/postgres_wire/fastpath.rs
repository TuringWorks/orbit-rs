//! Reading and writing fast-path arguments in text or binary.
//!
//! A `FunctionCall` message carries each argument as bytes plus a format code,
//! and asks for its result in a format too. Binary is not a variant of text:
//! an `int4` arrives as four big-endian bytes and would read as mojibake if
//! taken as a string, so the argument's declared type — read from the same
//! `pg_proc` entry the client took the OID from — is what makes decoding
//! possible rather than a guess.
//!
//! Everything here is pure: bytes and a type name in, SQL literal out.

use bytes::Bytes;

use super::plpgsql_function::normalize;
use crate::protocols::error::{ProtocolError, ProtocolResult};

/// The format code for text.
pub const TEXT_FORMAT: i16 = 0;
/// The format code for binary.
pub const BINARY_FORMAT: i16 = 1;

fn unsupported(what: &str, sql_type: &str) -> ProtocolError {
    ProtocolError::SqlState {
        code: "0A000",
        message: format!("{what} is not supported for type {sql_type} in a fast-path call"),
    }
}

fn malformed(sql_type: &str, wanted: usize, got: usize) -> ProtocolError {
    ProtocolError::SqlState {
        code: "22P03",
        message: format!("binary value for type {sql_type} is {got} byte(s), expected {wanted}"),
    }
}

/// Quote a value for substitution into `SELECT f(...)`.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// Turn one fast-path argument into the SQL literal for a call.
///
/// # Errors
/// Returns an error when a binary value is the wrong length for its type, or
/// when its type has no binary form this server reads.
pub fn decode_argument(
    value: Option<&[u8]>,
    format: i16,
    sql_type: &str,
) -> ProtocolResult<String> {
    let Some(bytes) = value else {
        return Ok("NULL".to_string());
    };

    if format != BINARY_FORMAT {
        // Text: a number goes in bare, anything else is quoted. The declared
        // type is not consulted, because in text form the value already reads
        // as what it is.
        let text = String::from_utf8_lossy(bytes);
        return Ok(if text.parse::<f64>().is_ok() {
            text.to_string()
        } else {
            quote(&text)
        });
    }

    let canonical = normalize(sql_type);
    let literal = match canonical.as_str() {
        "int2" => i16::from_be_bytes(
            bytes
                .try_into()
                .map_err(|_| malformed(&canonical, 2, bytes.len()))?,
        )
        .to_string(),
        "int4" => i32::from_be_bytes(
            bytes
                .try_into()
                .map_err(|_| malformed(&canonical, 4, bytes.len()))?,
        )
        .to_string(),
        "int8" => i64::from_be_bytes(
            bytes
                .try_into()
                .map_err(|_| malformed(&canonical, 8, bytes.len()))?,
        )
        .to_string(),
        "float4" => f32::from_be_bytes(
            bytes
                .try_into()
                .map_err(|_| malformed(&canonical, 4, bytes.len()))?,
        )
        .to_string(),
        "float8" => f64::from_be_bytes(
            bytes
                .try_into()
                .map_err(|_| malformed(&canonical, 8, bytes.len()))?,
        )
        .to_string(),
        "bool" => match bytes {
            [0] => "FALSE".to_string(),
            [_] => "TRUE".to_string(),
            other => return Err(malformed(&canonical, 1, other.len())),
        },
        "text" | "varchar" | "bpchar" => quote(&String::from_utf8_lossy(bytes)),
        // `numeric` has a binary form of digit groups with a weight and a
        // sign, and `date`/`timestamp` are offsets from an epoch that is not
        // the Unix one. Reading either approximately would corrupt the value
        // silently, which is worse than refusing.
        other => return Err(unsupported("binary input", other)),
    };
    Ok(literal)
}

/// Render a function's result in the format the client asked for.
///
/// # Errors
/// Returns an error when binary was asked for and the return type has no
/// binary form this server writes.
pub fn encode_result(
    value: Option<String>,
    format: i16,
    return_type: &str,
) -> ProtocolResult<Option<Bytes>> {
    let Some(text) = value else {
        return Ok(None);
    };

    if format != BINARY_FORMAT {
        return Ok(Some(Bytes::from(text.into_bytes())));
    }

    let canonical = normalize(return_type);
    let bytes = match canonical.as_str() {
        "int2" => Bytes::copy_from_slice(&parse::<i16>(&text, &canonical)?.to_be_bytes()),
        "int4" => Bytes::copy_from_slice(&parse::<i32>(&text, &canonical)?.to_be_bytes()),
        "int8" => Bytes::copy_from_slice(&parse::<i64>(&text, &canonical)?.to_be_bytes()),
        "float4" => Bytes::copy_from_slice(&parse::<f32>(&text, &canonical)?.to_be_bytes()),
        "float8" => Bytes::copy_from_slice(&parse::<f64>(&text, &canonical)?.to_be_bytes()),
        "bool" => Bytes::copy_from_slice(&[u8::from(matches!(
            text.trim(),
            "t" | "true" | "TRUE" | "True" | "1"
        ))]),
        "text" | "varchar" | "bpchar" => Bytes::from(text.into_bytes()),
        other => return Err(unsupported("binary output", other)),
    };
    Ok(Some(bytes))
}

/// Parse a value the server produced, which should always be well formed.
fn parse<T: std::str::FromStr>(text: &str, sql_type: &str) -> ProtocolResult<T> {
    text.trim()
        .parse::<T>()
        .map_err(|_| ProtocolError::SqlState {
            code: "22P03",
            message: format!("cannot render {text:?} as binary {sql_type}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_null_argument_is_null_in_either_format() {
        assert_eq!(
            decode_argument(None, TEXT_FORMAT, "INTEGER").expect("decodes"),
            "NULL"
        );
        assert_eq!(
            decode_argument(None, BINARY_FORMAT, "INTEGER").expect("decodes"),
            "NULL"
        );
    }

    #[test]
    fn text_numbers_go_in_bare_and_text_is_quoted() {
        assert_eq!(
            decode_argument(Some(b"42"), TEXT_FORMAT, "INTEGER").expect("decodes"),
            "42"
        );
        assert_eq!(
            decode_argument(Some(b"ada"), TEXT_FORMAT, "TEXT").expect("decodes"),
            "'ada'"
        );
        // A quote in the value must not end the literal.
        assert_eq!(
            decode_argument(Some(b"it's"), TEXT_FORMAT, "TEXT").expect("decodes"),
            "'it''s'"
        );
    }

    #[test]
    fn binary_integers_are_read_big_endian() {
        assert_eq!(
            decode_argument(Some(&42i32.to_be_bytes()), BINARY_FORMAT, "INTEGER").expect("decodes"),
            "42"
        );
        assert_eq!(
            decode_argument(Some(&(-7i64).to_be_bytes()), BINARY_FORMAT, "BIGINT")
                .expect("decodes"),
            "-7"
        );
        assert_eq!(
            decode_argument(Some(&300i16.to_be_bytes()), BINARY_FORMAT, "SMALLINT")
                .expect("decodes"),
            "300"
        );
    }

    #[test]
    fn a_binary_integer_read_as_text_would_be_nonsense() {
        // The reason the declared type is needed: these four bytes are not
        // the characters "42".
        let bytes = 42i32.to_be_bytes();
        let as_text = decode_argument(Some(&bytes), TEXT_FORMAT, "INTEGER").expect("decodes");
        assert_ne!(as_text, "42");
    }

    #[test]
    fn binary_booleans_and_strings_round_trip() {
        assert_eq!(
            decode_argument(Some(&[1]), BINARY_FORMAT, "BOOLEAN").expect("decodes"),
            "TRUE"
        );
        assert_eq!(
            decode_argument(Some(&[0]), BINARY_FORMAT, "BOOLEAN").expect("decodes"),
            "FALSE"
        );
        assert_eq!(
            decode_argument(Some(b"ada"), BINARY_FORMAT, "TEXT").expect("decodes"),
            "'ada'"
        );
    }

    #[test]
    fn a_binary_value_of_the_wrong_length_is_refused() {
        // Reading three bytes as an int4 would silently give a wrong number.
        let failure =
            decode_argument(Some(&[0, 0, 1]), BINARY_FORMAT, "INTEGER").expect_err("refused");
        assert!(failure.to_string().contains("expected 4"));
    }

    #[test]
    fn a_type_with_no_binary_reader_is_refused_not_guessed() {
        let failure =
            decode_argument(Some(&[1, 2, 3]), BINARY_FORMAT, "NUMERIC").expect_err("refused");
        assert!(failure.to_string().contains("not supported"));
    }

    #[test]
    fn a_text_result_is_the_bytes_of_the_value() {
        let encoded = encode_result(Some("42".to_string()), TEXT_FORMAT, "INTEGER")
            .expect("encodes")
            .expect("some");
        assert_eq!(&encoded[..], b"42");
    }

    #[test]
    fn a_binary_result_is_big_endian() {
        let encoded = encode_result(Some("42".to_string()), BINARY_FORMAT, "INTEGER")
            .expect("encodes")
            .expect("some");
        assert_eq!(&encoded[..], &42i32.to_be_bytes());
    }

    #[test]
    fn a_null_result_stays_null() {
        assert!(encode_result(None, BINARY_FORMAT, "INTEGER")
            .expect("encodes")
            .is_none());
    }

    #[test]
    fn a_binary_result_of_an_unwritable_type_is_refused() {
        let failure =
            encode_result(Some("1".to_string()), BINARY_FORMAT, "NUMERIC").expect_err("refused");
        assert!(failure.to_string().contains("not supported"));
    }

    #[test]
    fn a_round_trip_holds_for_every_type_with_a_binary_form() {
        for (sql_type, text) in [
            ("SMALLINT", "300"),
            ("INTEGER", "-42"),
            ("BIGINT", "5000000000"),
            ("FLOAT8", "1.5"),
            ("TEXT", "ada"),
        ] {
            let encoded = encode_result(Some(text.to_string()), BINARY_FORMAT, sql_type)
                .expect("encodes")
                .expect("some");
            let decoded =
                decode_argument(Some(&encoded), BINARY_FORMAT, sql_type).expect("decodes");
            let bare = decoded.trim_matches('\'');
            assert_eq!(bare, text, "{sql_type} did not survive a round trip");
        }
    }
}
