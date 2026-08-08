//! GSSAPI (Kerberos) authentication for the PostgreSQL wire protocol.
//!
//! The exchange is three messages wide. The server answers a startup packet
//! with `AuthenticationGSS`; the client replies with a token in a `'p'`
//! message; the server feeds that token to `gss_accept_sec_context` and either
//! answers `AuthenticationGSSContinue` with a token of its own and waits for
//! another, or — once the context is established — sends any final token and
//! then `AuthenticationOk`.
//!
//! # What this module does and does not decide
//!
//! None of the cryptography is here. Tokens are opaque: they are produced and
//! checked by the system Kerberos library against a real KDC, and this module
//! only carries them across the wire and asks the library who the caller
//! turned out to be. What *is* decided here is the part a library cannot
//! decide — whether the principal the KDC vouched for is allowed to log in as
//! the user named in the startup packet. That policy is
//! [`NameMapping::authorize`], a pure function, so it can be tested exhaustively
//! without a KDC in the loop.
//!
//! # The keytab
//!
//! Like PostgreSQL, this accepts with the *default* acceptor credential rather
//! than acquiring one for a named service, so any principal in the keytab can
//! be the target. The keytab is chosen by `KRB5_KTNAME`, read by the Kerberos
//! library itself — there is deliberately no second knob for it here, because a
//! knob that duplicates the library's own would be one more place for the two
//! to disagree.

use std::env;

use libgssapi::context::{SecurityContext, ServerCtx};

use crate::protocols::error::{ProtocolError, ProtocolResult};

/// The result of feeding one client token to the acceptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptStep {
    /// The handshake needs another round. Send this token as
    /// `AuthenticationGSSContinue` and wait for the client's reply.
    Continue(Vec<u8>),
    /// The context is established.
    ///
    /// `token` is not always empty when this arrives: under mutual
    /// authentication the last token is what proves the *server's* identity to
    /// the client, so it must still be sent — as `AuthenticationGSSContinue`,
    /// exactly as PostgreSQL does — before `AuthenticationOk`. Dropping it
    /// leaves a client that asked for mutual authentication waiting forever.
    Complete {
        token: Option<Vec<u8>>,
        principal: String,
    },
}

/// Why a principal was refused, kept separate from the message so the reasons
/// can be asserted on in tests without matching prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denial {
    /// The ticket is from a realm this server does not accept.
    WrongRealm { got: Option<String> },
    /// The principal authenticated, but is not this user.
    NotThisUser { principal: String, requested: String },
}

impl Denial {
    /// The message sent to the client.
    ///
    /// It says which principal was presented, because the usual cause is a
    /// stale ticket for someone else and a client that cannot see that spends
    /// a long time suspecting the password it never typed.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::WrongRealm { got } => match got {
                Some(realm) => format!("GSSAPI authentication failed: realm {realm:?} is not accepted by this server"),
                None => "GSSAPI authentication failed: the principal carries no realm, and this server requires one".to_string(),
            },
            Self::NotThisUser {
                principal,
                requested,
            } => format!(
                "GSSAPI authentication failed: principal {principal:?} is not authorized to log in as {requested:?}"
            ),
        }
    }
}

/// How an authenticated Kerberos principal is matched against the user named
/// in the startup packet.
///
/// These mirror PostgreSQL's `include_realm` and `krb_realm` settings, and the
/// defaults mirror its defaults: the realm is part of the name, and no
/// particular realm is required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameMapping {
    /// Whether the realm is part of the name being matched.
    ///
    /// With this on — the default, and PostgreSQL's since 9.5 — a client
    /// connecting as `ada` must present `ada@REALM` *and* be called
    /// `ada@REALM` in this server's user list. Turning it off compares only
    /// the part before the realm, which is convenient and is why it is not the
    /// default: with two trusted realms it lets `ada@OTHER.REALM` log in as
    /// `ada`.
    pub include_realm: bool,
    /// A realm that tickets must come from, if any.
    pub required_realm: Option<String>,
}

impl Default for NameMapping {
    fn default() -> Self {
        Self {
            include_realm: true,
            required_realm: None,
        }
    }
}

impl NameMapping {
    /// Read the policy from the environment.
    ///
    /// `ORBIT_PG_GSS_INCLUDE_REALM` accepts `0`/`false`/`off` to turn realm
    /// matching off; anything else, including absence, leaves it on. The
    /// asymmetry is deliberate — a typo must not silently loosen it.
    #[must_use]
    pub fn from_env() -> Self {
        let include_realm = env::var("ORBIT_PG_GSS_INCLUDE_REALM")
            .map(|value| !matches!(value.trim().to_ascii_lowercase().as_str(), "0" | "false" | "off" | "no"))
            .unwrap_or(true);
        let required_realm = env::var("ORBIT_PG_GSS_KRB_REALM")
            .ok()
            .map(|realm| realm.trim().to_string())
            .filter(|realm| !realm.is_empty());
        Self {
            include_realm,
            required_realm,
        }
    }

    /// Whether `principal` may log in as `requested`.
    ///
    /// # Errors
    /// Returns the reason the principal was refused.
    pub fn authorize(&self, principal: &str, requested: &str) -> Result<(), Denial> {
        // A principal is `name@REALM`, and the name itself may contain `/`
        // (`postgres/host`) but not `@` — so the realm is what follows the
        // last one.
        let (name, realm) = match principal.rsplit_once('@') {
            Some((name, realm)) => (name, Some(realm)),
            None => (principal, None),
        };

        if let Some(required) = &self.required_realm {
            // Kerberos realms are conventionally upper case but are compared
            // by the KDC as written, so this compares as written too.
            if realm != Some(required.as_str()) {
                return Err(Denial::WrongRealm {
                    got: realm.map(ToString::to_string),
                });
            }
        }

        let candidate = if self.include_realm { principal } else { name };
        if candidate == requested {
            Ok(())
        } else {
            Err(Denial::NotThisUser {
                principal: principal.to_string(),
                requested: requested.to_string(),
            })
        }
    }
}

/// One connection's half-finished GSSAPI handshake.
pub struct Acceptor {
    context: ServerCtx,
}

impl std::fmt::Debug for Acceptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The context holds key material; there is nothing safe to print.
        f.write_str("Acceptor { .. }")
    }
}

impl Default for Acceptor {
    fn default() -> Self {
        Self::new()
    }
}

fn failed(context: &str, error: &libgssapi::error::Error) -> ProtocolError {
    ProtocolError::SqlState {
        // 28000 invalid_authorization_specification, which is what PostgreSQL
        // reports for a failed login.
        code: "28000",
        message: format!("GSSAPI authentication failed: {context}: {error}"),
    }
}

impl Acceptor {
    /// Start a handshake using the default acceptor credential.
    #[must_use]
    pub fn new() -> Self {
        // `None` is GSS_C_NO_CREDENTIAL: accept as any principal the keytab
        // holds a key for. This is what PostgreSQL does, and it is why a
        // server principal does not have to be configured twice.
        Self {
            context: ServerCtx::new(None),
        }
    }

    /// Feed one token from the client.
    ///
    /// # Errors
    /// Returns an error when the token is rejected by the Kerberos library —
    /// a forged or replayed ticket, a key the keytab does not hold, or a
    /// clock too far out of step with the KDC.
    pub fn step(&mut self, token: &[u8]) -> ProtocolResult<AcceptStep> {
        let outgoing = self
            .context
            .step(token, None)
            .map_err(|error| failed("accepting the security context", &error))?;
        let token = outgoing.map(|buf| buf.to_vec());

        if !self.context.is_complete() {
            // Not established yet, so there must be something to send back;
            // if there is not, the handshake cannot advance and would hang.
            return match token {
                Some(token) => Ok(AcceptStep::Continue(token)),
                None => Err(ProtocolError::SqlState {
                    code: "28000",
                    message: "GSSAPI authentication failed: the mechanism asked for another round but produced no token".to_string(),
                }),
            };
        }

        let principal = self
            .context
            .source_name()
            .map_err(|error| failed("reading the client principal", &error))?
            .to_string();

        Ok(AcceptStep::Complete { token, principal })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The authorization policy is the part of GSSAPI this server decides for
    // itself, so it is the part tested exhaustively. The handshake proper is
    // exercised against a real KDC by the conformance harness.

    fn default_policy() -> NameMapping {
        NameMapping::default()
    }

    #[test]
    fn the_realm_is_part_of_the_name_by_default() {
        let policy = default_policy();
        assert_eq!(policy.authorize("ada@ORBIT.TEST", "ada@ORBIT.TEST"), Ok(()));
        // The bare name is not enough, which is PostgreSQL's default and the
        // reason two trusted realms cannot impersonate each other.
        assert_eq!(
            policy.authorize("ada@ORBIT.TEST", "ada"),
            Err(Denial::NotThisUser {
                principal: "ada@ORBIT.TEST".to_string(),
                requested: "ada".to_string(),
            })
        );
    }

    #[test]
    fn without_the_realm_the_bare_name_matches() {
        let policy = NameMapping {
            include_realm: false,
            required_realm: None,
        };
        assert_eq!(policy.authorize("ada@ORBIT.TEST", "ada"), Ok(()));
        // And someone else still does not.
        assert!(policy.authorize("eve@ORBIT.TEST", "ada").is_err());
    }

    #[test]
    fn without_the_realm_any_realm_would_do_which_is_why_it_is_not_the_default() {
        // This is the documented hazard, asserted so it cannot quietly change:
        // realm matching off and no required realm means a principal from
        // another trusted realm logs in as the same name.
        let policy = NameMapping {
            include_realm: false,
            required_realm: None,
        };
        assert_eq!(policy.authorize("ada@EVIL.TEST", "ada"), Ok(()));

        // Requiring a realm is the fix, and it works.
        let guarded = NameMapping {
            include_realm: false,
            required_realm: Some("ORBIT.TEST".to_string()),
        };
        assert_eq!(
            guarded.authorize("ada@EVIL.TEST", "ada"),
            Err(Denial::WrongRealm {
                got: Some("EVIL.TEST".to_string())
            })
        );
        assert_eq!(guarded.authorize("ada@ORBIT.TEST", "ada"), Ok(()));
    }

    #[test]
    fn a_service_principal_keeps_its_slash() {
        // `postgres/host@REALM` splits at the last `@`, not the first `/`.
        let policy = NameMapping {
            include_realm: false,
            required_realm: None,
        };
        assert_eq!(policy.authorize("postgres/localhost@ORBIT.TEST", "postgres/localhost"), Ok(()));
    }

    #[test]
    fn a_principal_with_no_realm_fails_a_realm_requirement() {
        let policy = NameMapping {
            include_realm: false,
            required_realm: Some("ORBIT.TEST".to_string()),
        };
        assert_eq!(
            policy.authorize("ada", "ada"),
            Err(Denial::WrongRealm { got: None })
        );
    }

    #[test]
    fn matching_is_case_sensitive() {
        // Kerberos principals are case sensitive, and folding them here would
        // make `ADA` and `ada` the same login when the KDC says they are not.
        let policy = default_policy();
        assert!(policy.authorize("ADA@ORBIT.TEST", "ada@ORBIT.TEST").is_err());
        assert!(policy.authorize("ada@orbit.test", "ada@ORBIT.TEST").is_err());
    }

    #[test]
    fn an_empty_requested_user_matches_nothing() {
        let policy = default_policy();
        assert!(policy.authorize("ada@ORBIT.TEST", "").is_err());
    }

    #[test]
    fn a_denial_names_the_principal_it_refused() {
        let denial = Denial::NotThisUser {
            principal: "eve@ORBIT.TEST".to_string(),
            requested: "ada".to_string(),
        };
        let message = denial.message();
        assert!(message.contains("eve@ORBIT.TEST"), "{message}");
        assert!(message.contains("ada"), "{message}");
    }
}
