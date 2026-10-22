//! SQLSTATE codes for the errors this server reports.
//!
//! Every error used to leave as `XX000` — `internal_error`. That is the code
//! PostgreSQL uses for "something went wrong that we cannot name", and drivers
//! treat it accordingly: an application could not tell a duplicate key from a
//! crashed backend, and every `ON CONFLICT`-style retry loop, every ORM's
//! "is this a unique violation?" branch, and every PL/pgSQL `WHEN
//! unique_violation` was answered with the same shrug.
//!
//! # Why this classifies text
//!
//! The right shape is a code at every raise site. There are several hundred of
//! them, and a half-converted error type would be worse than none: some codes
//! honest, others silently still `XX000`, with no way to tell which from the
//! outside. So the mapping lives here, in one place, keyed on the message text
//! the engine itself produces.
//!
//! That makes this a contract between the raise sites and this table, and such
//! contracts drift. The guard is that every condition below is triggered
//! end-to-end by a check in the conformance harness, which asserts the code a
//! real client receives — so a reworded message shows up as a failing check
//! rather than as a silent return to `XX000`.

/// `internal_error` — nothing more specific is known.
pub const INTERNAL: &str = "XX000";

/// Codes this server can name, with the phrase that identifies each.
///
/// Order matters: the first match wins, so a more specific phrase must come
/// before a more general one that also matches it.
const CONDITIONS: &[(&str, &str)] = &[
    // Class 23 — integrity constraint violation.
    ("violates not-null constraint", "23502"),
    ("violates unique constraint", "23505"),
    ("violates foreign key constraint", "23503"),
    ("violates check constraint", "23514"),
    // Class 22 — data exception.
    ("division by zero", "22012"),
    ("invalid input syntax", "22P02"),
    // Class 42 — syntax error or access rule violation.
    ("already exists", "42P07"),
    ("does not exist", "42P01"),
    ("is not unique", "42725"),
    ("not implemented", "42883"),
    ("unknown function", "42883"),
    ("syntax error", "42601"),
    ("parse error", "42601"),
    // Class 40 — transaction rollback.
    ("is not supported", "0A000"),
    ("could not serialize access", "40001"),
    // Class 57 — operator intervention.
    ("canceling statement due to user request", "57014"),
];

/// A column being missing is `undefined_column`, not `undefined_table`, and
/// both are phrased "does not exist".
const COLUMN_PHRASES: &[&str] = &["column"];

/// A function being missing is `undefined_function`.
const FUNCTION_PHRASES: &[&str] = &["function"];

/// The SQLSTATE code for an error message.
///
/// Falls back to [`INTERNAL`], which is what an unrecognised error genuinely
/// is: unclassified. Returning a plausible-looking code for an error nobody
/// has categorised would be worse than admitting it.
#[must_use]
pub fn classify(message: &str) -> &'static str {
    let lowered = message.to_lowercase();

    for (phrase, code) in CONDITIONS {
        if !lowered.contains(phrase) {
            continue;
        }
        // "does not exist" covers tables, columns and functions, which are
        // three different codes.
        if *phrase == "does not exist" {
            if COLUMN_PHRASES.iter().any(|p| lowered.contains(p)) {
                return "42703";
            }
            if FUNCTION_PHRASES.iter().any(|p| lowered.contains(p)) {
                return "42883";
            }
        }
        return code;
    }

    INTERNAL
}

/// Whether a PL/pgSQL condition name matches a SQLSTATE code.
///
/// This is what lets `WHEN unique_violation THEN` catch the right failure
/// rather than everything or nothing.
#[must_use]
pub fn condition_matches(condition: &str, code: &str) -> bool {
    if condition.eq_ignore_ascii_case("OTHERS") {
        return true;
    }
    condition_code(condition).is_some_and(|expected| expected == code)
}

/// The SQLSTATE code a PL/pgSQL condition name stands for.
#[must_use]
pub fn condition_code(condition: &str) -> Option<&'static str> {
    let name = condition.to_lowercase();
    Some(match name.as_str() {
        "not_null_violation" => "23502",
        "unique_violation" => "23505",
        "foreign_key_violation" => "23503",
        "check_violation" => "23514",
        "integrity_constraint_violation" => "23000",
        "division_by_zero" => "22012",
        "invalid_text_representation" => "22P02",
        "undefined_table" => "42P01",
        "undefined_column" => "42703",
        "undefined_function" => "42883",
        "ambiguous_function" => "42725",
        "duplicate_table" => "42P07",
        "syntax_error" => "42601",
        "serialization_failure" => "40001",
        "query_canceled" => "57014",
        "raise_exception" => "P0001",
        "feature_not_supported" => "0A000",
        "internal_error" => INTERNAL,
        _ => return None,
    })
}

/// The SQLSTATE an error should be reported under.
///
/// An error that carries its own code keeps it; anything else is classified
/// from its message.
#[must_use]
pub fn of(error: &crate::protocols::error::ProtocolError) -> &'static str {
    match error {
        crate::protocols::error::ProtocolError::SqlState { code, .. } => code,
        other => classify(&other.to_string()),
    }
}

/// The code a `RAISE EXCEPTION` reports.
///
/// PostgreSQL uses `P0001` for an exception raised by a procedure, which is
/// what makes `WHEN raise_exception` work.
pub const RAISE_EXCEPTION: &str = "P0001";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constraint_violations_get_their_own_codes() {
        assert_eq!(
            classify("null value in column \"a\" violates not-null constraint"),
            "23502"
        );
        assert_eq!(
            classify("duplicate key value violates unique constraint on column \"a\""),
            "23505"
        );
        assert_eq!(
            classify("insert or update violates foreign key constraint: no row in \"b\""),
            "23503"
        );
        assert_eq!(
            classify("new row violates check constraint on column \"a\""),
            "23514"
        );
    }

    #[test]
    fn a_missing_table_column_and_function_are_told_apart() {
        assert_eq!(classify("Table 'x' does not exist"), "42P01");
        assert_eq!(classify("column \"x\" does not exist"), "42703");
        assert_eq!(classify("Function 'X' not implemented"), "42883");
    }

    #[test]
    fn an_ambiguous_call_is_its_own_condition() {
        // 42725 is ambiguous_function; reporting it as undefined_function
        // would tell a caller the function is missing when it is the choice
        // between two of them that failed.
        assert_eq!(classify("function f(unknown) is not unique"), "42725");
        assert_eq!(classify("function f(bool) does not exist"), "42883");
    }

    #[test]
    fn a_duplicate_table_is_not_a_missing_one() {
        assert_eq!(classify("Table 'x' already exists"), "42P07");
    }

    #[test]
    fn transaction_and_cancellation_codes_are_named() {
        assert_eq!(
            classify("could not serialize access due to concurrent update on \"t\""),
            "40001"
        );
        assert_eq!(classify("canceling statement due to user request"), "57014");
    }

    #[test]
    fn an_unsupported_feature_is_not_an_internal_error() {
        // A client must be able to tell a feature this server does not have
        // from a backend that fell over.
        assert_eq!(
            classify("physical replication is not supported; use ..."),
            "0A000"
        );
    }

    #[test]
    fn an_unrecognised_error_stays_unclassified() {
        // Guessing a plausible code for an uncategorised error would be worse
        // than admitting it is uncategorised.
        assert_eq!(classify("the disk caught fire"), INTERNAL);
    }

    #[test]
    fn a_condition_name_matches_only_its_own_code() {
        assert!(condition_matches("unique_violation", "23505"));
        assert!(!condition_matches("unique_violation", "23502"));
        assert!(condition_matches("OTHERS", "23502"));
        // A name nobody defined matches nothing rather than everything.
        assert!(!condition_matches("no_such_condition", "23505"));
    }
}
