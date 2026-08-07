//! Calling a stored PL/pgSQL function from inside a query.
//!
//! A stored function could only be called as a bare `SELECT f(literal)`. The
//! query engine intercepts that shape before the expression evaluator ever
//! sees it, so anything else went to the evaluator, which had never heard of
//! the function: `SELECT f(id) FROM t` failed with
//! `Function 'F' not implemented`, and — worse — `WHERE f(id) = 4` returned no
//! rows instead of failing, which is a wrong answer rather than a missing
//! feature.
//!
//! # Why only some functions
//!
//! The evaluator is synchronous and the query engine is not, so a body that
//! runs SQL cannot be executed from here without blocking a runtime worker.
//! Only a **pure** body is registered — one whose statements are assignments,
//! conditionals, loops and `RETURN` over expressions, with no SQL statement in
//! it. That covers the scalar functions people write to use in a `SELECT` list
//! or a `WHERE`, and a body that does run SQL keeps the old error rather than
//! being called in a way that could deadlock.
//!
//! The invariant: an entry is written only by the query engine, when a
//! function is created and once at startup for those already stored, and is
//! keyed by name and argument count.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use super::plpgsql::{Block, Stmt};
use super::plpgsql_function::Parameter;

/// A function this module can run without leaving the evaluator.
#[derive(Debug, Clone)]
pub struct PureFunction {
    /// Its parameters, in declared order.
    pub parameters: Vec<Parameter>,
    /// Its parsed body.
    pub block: Arc<Block>,
}

/// Candidates share a name and argument count; which one a call means is
/// decided by the argument types, exactly as it is for a direct call.
type Registry = RwLock<HashMap<(String, usize), Vec<PureFunction>>>;

static FUNCTIONS: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
    FUNCTIONS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Whether a body can be run without a database — no statement in it is SQL.
///
/// Conservative by construction: a statement this does not recognise as pure
/// makes the whole body impure, so a new statement kind is excluded until
/// someone decides otherwise.
#[must_use]
pub fn is_pure(block: &Block) -> bool {
    statements_are_pure(&block.body)
        && block
            .handlers
            .iter()
            .all(|handler| statements_are_pure(&handler.body))
}

fn statements_are_pure(body: &[Stmt]) -> bool {
    body.iter().all(|statement| match statement {
        Stmt::Assign { .. } | Stmt::Return { .. } | Stmt::Raise { .. } | Stmt::Nothing => true,
        Stmt::Exit { .. } | Stmt::Continue { .. } => true,
        Stmt::If {
            branches,
            otherwise,
        } => {
            branches.iter().all(|(_, body)| statements_are_pure(body))
                && statements_are_pure(otherwise)
        }
        Stmt::While { body, .. } | Stmt::Loop { body } | Stmt::ForRange { body, .. } => {
            statements_are_pure(body)
        }
        Stmt::Nested { block } => is_pure(block),
        // Everything else touches the database: a bare SQL statement, a
        // `SELECT ... INTO`, a cursor, a query loop, or `RETURN QUERY`.
        _ => false,
    })
}

/// Register a function that can be called from an expression.
///
/// A body that is not pure is *removed* rather than ignored, so replacing a
/// pure function with one that runs SQL does not leave the old one callable.
pub fn remember(name: &str, parameters: Vec<Parameter>, block: Block) {
    let arity = super::plpgsql_function::input_arity(&parameters);
    let key = (name.to_lowercase(), arity);
    let signature = super::plpgsql_function::signature(&parameters);

    let Ok(mut functions) = registry().write() else {
        return;
    };
    let candidates = functions.entry(key).or_default();
    // One entry per signature: redefining a function replaces it, and does
    // not leave the old body callable alongside the new one.
    candidates
        .retain(|existing| super::plpgsql_function::signature(&existing.parameters) != signature);

    if is_pure(&block) {
        candidates.push(PureFunction {
            parameters,
            block: Arc::new(block),
        });
    }
}

/// Forget every arity of a name that has been dropped.
pub fn forget(name: &str) {
    let name = name.to_lowercase();
    if let Ok(mut functions) = registry().write() {
        functions.retain(|(stored, _), _| *stored != name);
    }
}

/// Every pure function of this name taking this many arguments.
#[must_use]
pub fn candidates(name: &str, arity: usize) -> Vec<PureFunction> {
    registry()
        .read()
        .ok()
        .and_then(|functions| functions.get(&(name.to_lowercase(), arity)).cloned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::postgres_wire::plpgsql;

    fn block(source: &str) -> Block {
        plpgsql::parse(source).unwrap_or_else(|e| panic!("parse {source}: {e}"))
    }

    #[test]
    fn a_body_of_expressions_is_pure() {
        assert!(is_pure(&block("BEGIN RETURN 1; END")));
        assert!(is_pure(&block(
            "DECLARE n INTEGER := 0; BEGIN WHILE n < 3 LOOP n := n + 1; END LOOP; RETURN n; END"
        )));
        assert!(is_pure(&block(
            "BEGIN IF 1 > 0 THEN RETURN 'a'; ELSE RETURN 'b'; END IF; END"
        )));
        assert!(is_pure(&block(
            "BEGIN FOR i IN 1..3 LOOP EXIT WHEN i > 2; END LOOP; RETURN 1; END"
        )));
    }

    #[test]
    fn a_nested_block_is_checked_too() {
        assert!(is_pure(&block("BEGIN BEGIN RETURN 1; END; END")));
        // A nested block must not smuggle a SQL statement past the check.
        assert!(!is_pure(&block(
            "BEGIN BEGIN INSERT INTO t VALUES (1); END; END"
        )));
    }

    #[test]
    fn a_body_that_touches_the_database_is_not() {
        assert!(!is_pure(&block("BEGIN INSERT INTO t VALUES (1); END")));
        assert!(!is_pure(&block("BEGIN SELECT COUNT(*) FROM t INTO n; END")));
        assert!(!is_pure(&block("BEGIN RETURN QUERY SELECT a FROM t; END")));
        // Nested inside a branch counts too, or a conditional would smuggle
        // one past.
        assert!(!is_pure(&block(
            "BEGIN IF 1 > 0 THEN INSERT INTO t VALUES (1); END IF; END"
        )));
        // And inside a loop.
        assert!(!is_pure(&block(
            "BEGIN FOR i IN 1..3 LOOP INSERT INTO t VALUES (i); END LOOP; END"
        )));
    }

    #[test]
    fn registering_an_impure_body_removes_any_pure_one() {
        let parameters = super::super::plpgsql_function::parse_parameters("a INTEGER");
        remember(
            "test_swap",
            parameters.clone(),
            block("BEGIN RETURN 1; END"),
        );
        assert_eq!(candidates("test_swap", 1).len(), 1);

        // Replacing it with a body that runs SQL must not leave the old one
        // callable from an expression.
        remember(
            "test_swap",
            parameters,
            block("BEGIN INSERT INTO t VALUES (1); END"),
        );
        assert!(candidates("test_swap", 1).is_empty());
        forget("test_swap");
    }

    #[test]
    fn two_overloads_of_one_arity_both_survive() {
        // Keyed by arity alone, the second replaced the first, and a call
        // inside a query reached whichever was defined last whatever its
        // argument was.
        let numeric = super::super::plpgsql_function::parse_parameters("a INTEGER");
        let textual = super::super::plpgsql_function::parse_parameters("a TEXT");
        remember("test_both", numeric, block("BEGIN RETURN 'i'; END"));
        remember("test_both", textual, block("BEGIN RETURN 't'; END"));
        assert_eq!(candidates("test_both", 1).len(), 2);
        forget("test_both");
    }

    #[test]
    fn redefining_one_signature_replaces_only_it() {
        let numeric = super::super::plpgsql_function::parse_parameters("a INTEGER");
        let textual = super::super::plpgsql_function::parse_parameters("a TEXT");
        remember(
            "test_replace",
            numeric.clone(),
            block("BEGIN RETURN 1; END"),
        );
        remember("test_replace", textual, block("BEGIN RETURN 2; END"));
        remember("test_replace", numeric, block("BEGIN RETURN 3; END"));
        assert_eq!(candidates("test_replace", 1).len(), 2);
        forget("test_replace");
    }

    #[test]
    fn arity_is_part_of_the_key() {
        let one = super::super::plpgsql_function::parse_parameters("a INTEGER");
        let two = super::super::plpgsql_function::parse_parameters("a INTEGER, b INTEGER");
        remember("test_arity", one, block("BEGIN RETURN 1; END"));
        remember("test_arity", two, block("BEGIN RETURN 2; END"));

        assert_eq!(candidates("test_arity", 1).len(), 1);
        assert_eq!(candidates("test_arity", 2).len(), 1);
        assert!(candidates("test_arity", 3).is_empty());
        forget("test_arity");
    }

    #[test]
    fn dropping_forgets_every_arity() {
        let one = super::super::plpgsql_function::parse_parameters("a INTEGER");
        let two = super::super::plpgsql_function::parse_parameters("a INTEGER, b INTEGER");
        remember("test_drop", one, block("BEGIN RETURN 1; END"));
        remember("test_drop", two, block("BEGIN RETURN 2; END"));
        forget("test_drop");
        assert!(candidates("test_drop", 1).is_empty());
        assert!(candidates("test_drop", 2).is_empty());
    }
}
