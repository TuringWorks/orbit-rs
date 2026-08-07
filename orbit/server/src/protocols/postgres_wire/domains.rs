//! What each domain is built on, for code that cannot reach the catalogue.
//!
//! A cast names a type: `42::posint`. Resolving that name needs the
//! catalogue, and the expression evaluator has none — `SqlValue::cast_to` is a
//! pure function over a value and a type, and it refused every cast to a
//! domain with `Cannot cast Integer to Custom { .. }`. Casting to a domain is
//! ordinary SQL, so the alternative to this registry was leaving it broken.
//!
//! The invariant: an entry maps a domain's name, folded to lower case, to the
//! type it is built on, and is written only by the query engine — when a
//! domain is created, and once at startup for the domains already stored. A
//! name that is not here is not known to be a domain, and a cast to it still
//! fails rather than passing the value through. That direction matters: a
//! typo'd type name must not silently succeed.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

type Registry = RwLock<HashMap<String, String>>;

static DOMAINS: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
    DOMAINS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Record that `name` is a domain over `base_type`.
pub fn remember(name: &str, base_type: &str) {
    if base_type.trim().is_empty() {
        return;
    }
    if let Ok(mut domains) = registry().write() {
        domains.insert(name.to_lowercase(), base_type.trim().to_string());
    }
}

/// Forget a domain that has been dropped.
pub fn forget(name: &str) {
    if let Ok(mut domains) = registry().write() {
        domains.remove(&name.to_lowercase());
    }
}

/// The type `name` is built on, if it is a known domain.
#[must_use]
pub fn base_of(name: &str) -> Option<String> {
    registry()
        .read()
        .ok()
        .and_then(|domains| domains.get(&name.to_lowercase()).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recorded_domain_reports_its_base_type() {
        remember("Test_PosInt", "INTEGER");
        // Names fold, as identifiers do everywhere else.
        assert_eq!(base_of("test_posint").as_deref(), Some("INTEGER"));
        assert_eq!(base_of("TEST_POSINT").as_deref(), Some("INTEGER"));
        forget("test_posint");
    }

    #[test]
    fn an_unknown_name_is_not_a_domain() {
        // The important direction: a typo must not resolve to something.
        assert!(base_of("test_no_such_domain_anywhere").is_none());
    }

    #[test]
    fn a_dropped_domain_is_forgotten() {
        remember("test_gone", "TEXT");
        forget("test_gone");
        assert!(base_of("test_gone").is_none());
    }

    #[test]
    fn an_empty_base_type_is_not_recorded() {
        // Recording one would make a cast to it succeed while resolving to
        // nothing, which is worse than not knowing the domain at all.
        remember("test_empty", "   ");
        assert!(base_of("test_empty").is_none());
    }
}
