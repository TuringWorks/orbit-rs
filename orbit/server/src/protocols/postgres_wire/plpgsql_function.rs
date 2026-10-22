//! A stored PL/pgSQL function's parameters: parsing, storing and resolving.
//!
//! Split out of the query engine because three separate things needed it and
//! all three got it slightly wrong when it was inline: the parameter list was
//! split on commas, so `NUMERIC(10, 2)` became two parameters; the stored form
//! was joined on commas, so reading it back split the same type again; and a
//! call was resolved by argument *count* alone, so two functions of one name
//! and arity could not coexist.

use crate::protocols::error::{ProtocolError, ProtocolResult};

/// Separates parameters in the stored form.
///
/// A control character rather than a comma: a declared type may contain a
/// comma (`NUMERIC(10, 2)`), and joining on one made the stored form
/// unreadable.
const PARAMETER_SEPARATOR: char = '\u{1}';

/// Separates a parameter's fields in the stored form.
const FIELD_SEPARATOR: char = '\u{2}';

/// How a parameter passes its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Mode {
    /// Passed in by the caller. The default when nothing is written.
    #[default]
    In,
    /// Set by the body and returned to the caller.
    Out,
    /// Both.
    InOut,
}

impl Mode {
    /// Whether a caller supplies this parameter.
    #[must_use]
    pub fn is_input(self) -> bool {
        matches!(self, Mode::In | Mode::InOut)
    }

    /// Whether this parameter is part of what the function returns.
    #[must_use]
    pub fn is_output(self) -> bool {
        matches!(self, Mode::Out | Mode::InOut)
    }

    fn as_str(self) -> &'static str {
        match self {
            Mode::In => "i",
            Mode::Out => "o",
            Mode::InOut => "b",
        }
    }

    fn from_str(text: &str) -> Self {
        match text {
            "o" => Mode::Out,
            "b" => Mode::InOut,
            _ => Mode::In,
        }
    }
}

/// One declared parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Its name, folded to lower case.
    pub name: String,
    /// Its declared type, upper-cased, as written.
    pub sql_type: String,
    /// How it passes its value.
    pub mode: Mode,
}

/// What kind of thing a type is.
///
/// PostgreSQL groups types into categories and resolves an overload within
/// one; a numeric argument never selects a string parameter however few
/// candidates remain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Category {
    /// `int2`, `int4`, `int8`, `numeric`, `float4`, `float8`.
    Numeric,
    /// `text`, `varchar`, `char`.
    String,
    /// `bool`.
    Boolean,
    /// `date`, `time`, `timestamp`, `timestamptz`.
    DateTime,
    /// An array of some element type.
    Array,
    /// Anything else, which matches only itself.
    Other,
}

/// A declared type, reduced to the canonical name PostgreSQL uses.
///
/// `INTEGER` and `INT4` are the same type written two ways; resolution has to
/// see them as one, and as different from `INT8`.
///
/// A name this does not recognise keeps its own spelling rather than becoming
/// `text`. Collapsing it made every user-defined type — a composite, an enum,
/// anything — the same type as `text`, so `f(mytype)` and `f(text)` could not
/// both exist and a call to one could reach the other.
#[must_use]
pub fn normalize(sql_type: &str) -> String {
    normalize_known(sql_type).map_or_else(
        || {
            sql_type
                .split('(')
                .next()
                .unwrap_or(sql_type)
                .trim()
                .to_lowercase()
        },
        str::to_string,
    )
}

/// Whether this is a built-in scalar type rather than a name to look up.
///
/// Used to avoid a catalogue lookup for every `DECLARE n INTEGER`, and to keep
/// a built-in name from being read as a composite because something else in
/// the database happens to share it.
#[must_use]
pub fn is_builtin(sql_type: &str) -> bool {
    normalize_known(sql_type).is_some() || array_element(sql_type).is_some()
}

/// The canonical name of a type this module knows, if it is one.
#[must_use]
fn normalize_known(sql_type: &str) -> Option<&'static str> {
    let written = sql_type.trim();

    // `int4[]` and `int4 ARRAY` are the same type, and an array carries its
    // element type: collapsing every array to one kind made `f(int4[])` and
    // `f(text[])` the same signature, so the second replaced the first.
    // `_int4` is what PostgreSQL calls the array over `int4`.
    if let Some(element) = array_element(written) {
        return Some(match normalize(element).as_str() {
            "int2" => "_int2",
            "int4" => "_int4",
            "int8" => "_int8",
            "float4" => "_float4",
            "float8" => "_float8",
            "numeric" => "_numeric",
            "bool" => "_bool",
            "varchar" => "_varchar",
            "bpchar" => "_bpchar",
            "date" => "_date",
            "time" => "_time",
            "timestamp" => "_timestamp",
            "timestamptz" => "_timestamptz",
            _ => "_text",
        });
    }

    let bare = written
        .split('(')
        .next()
        .unwrap_or(written)
        .trim()
        .to_uppercase();
    Some(match bare.as_str() {
        "INT2" | "SMALLINT" => "int2",
        "INT" | "INT4" | "INTEGER" | "SERIAL" => "int4",
        "INT8" | "BIGINT" | "BIGSERIAL" => "int8",
        "NUMERIC" | "DECIMAL" => "numeric",
        "REAL" | "FLOAT4" => "float4",
        "DOUBLE" | "DOUBLE PRECISION" | "FLOAT" | "FLOAT8" => "float8",
        "BOOL" | "BOOLEAN" => "bool",
        "CHAR" | "CHARACTER" | "BPCHAR" => "bpchar",
        "VARCHAR" | "CHARACTER VARYING" => "varchar",
        "TEXT" => "text",
        "DATE" => "date",
        "TIME" => "time",
        "TIMESTAMP" => "timestamp",
        "TIMESTAMPTZ" | "TIMESTAMP WITH TIME ZONE" => "timestamptz",
        "JSON" => "json",
        "JSONB" => "jsonb",
        "UUID" => "uuid",
        "BYTEA" => "bytea",
        "" => UNKNOWN,
        // Anything else keeps its own name, decided by the caller.
        _ => return None,
    })
}

/// The element type of an array declaration, if it is one.
///
/// `INTEGER[]` and `INTEGER ARRAY` both name an array over `INTEGER`.
#[must_use]
pub fn array_element(sql_type: &str) -> Option<&str> {
    let written = sql_type.trim();
    if let Some(element) = written.strip_suffix("[]") {
        return Some(element.trim());
    }
    // Case-insensitively, without allocating a lowered copy of the whole type.
    let upper = written.to_uppercase();
    upper
        .strip_suffix(" ARRAY")
        .map(|kept| written[..kept.len()].trim())
}

/// The type of a value nobody has assigned a type to yet.
///
/// A quoted literal in SQL is `unknown` until context gives it a type, which
/// is what lets `f('5')` select either `f(text)` or `f(int4)` depending on
/// what exists.
pub const UNKNOWN: &str = "unknown";

/// Which category a canonical type belongs to.
#[must_use]
pub fn category(canonical: &str) -> Category {
    match canonical {
        "int2" | "int4" | "int8" | "numeric" | "float4" | "float8" => Category::Numeric,
        "text" | "varchar" | "bpchar" => Category::String,
        "bool" => Category::Boolean,
        "date" | "time" | "timestamp" | "timestamptz" => Category::DateTime,
        name if name.starts_with('_') => Category::Array,
        _ => Category::Other,
    }
}

/// How wide a type is within its category.
///
/// A value converts implicitly to a type of the same category and equal or
/// higher rank — `int4` to `int8` but not the reverse, which is what stops a
/// call losing precision without being asked.
#[must_use]
pub fn rank(canonical: &str) -> u8 {
    match canonical {
        "int2" => 1,
        "int4" => 2,
        "int8" => 3,
        "numeric" => 4,
        "float4" => 5,
        "float8" => 6,
        "bpchar" => 1,
        "varchar" => 2,
        "text" => 3,
        "time" => 1,
        "date" => 2,
        "timestamp" => 3,
        "timestamptz" => 4,
        _ => 0,
    }
}

/// Whether this is the type its category prefers.
///
/// PostgreSQL breaks a remaining tie towards the preferred type: `int4` among
/// the integers, `text` among the strings, `timestamptz` among the times.
#[must_use]
pub fn is_preferred(canonical: &str) -> bool {
    matches!(
        canonical,
        "int4" | "text" | "float8" | "timestamptz" | "bool"
    )
}

/// Whether a value of `given` may be passed where `wanted` is declared.
#[must_use]
pub fn converts_to(given: &str, wanted: &str) -> bool {
    if given == UNKNOWN || wanted == UNKNOWN || given == wanted {
        return true;
    }
    // An array converts only to the identical array type: widening an
    // `int4[]` into an `int8[]` would mean rebuilding every element, which
    // nothing here does.
    if category(given) == Category::Array || category(wanted) == Category::Array {
        return false;
    }

    // Same category, and not narrowing.
    category(given) == category(wanted)
        && category(given) != Category::Other
        && rank(given) <= rank(wanted)
}

/// The type of an argument, from how it was written and what it evaluated to.
///
/// A quoted literal stays [`UNKNOWN`] — that is what PostgreSQL does, and it
/// is what lets one literal fit either of two overloads. An integer literal
/// too large for `int4` is `int8`, as PostgreSQL also promotes it.
#[must_use]
pub fn argument_type(source: &str, value: Option<&str>) -> &'static str {
    let written = source.trim();

    if written.starts_with('\'') || written.starts_with('"') {
        return UNKNOWN;
    }
    if written.eq_ignore_ascii_case("NULL") {
        return UNKNOWN;
    }
    if written.eq_ignore_ascii_case("TRUE") || written.eq_ignore_ascii_case("FALSE") {
        return "bool";
    }
    if let Ok(whole) = written.parse::<i64>() {
        return if i32::try_from(whole).is_ok() {
            "int4"
        } else {
            "int8"
        };
    }
    if written.parse::<f64>().is_ok() {
        return "numeric";
    }

    // Not a literal: fall back to what it evaluated to. An expression's type
    // is the server's to know, and this only sees its printed value.
    match value {
        None => UNKNOWN,
        Some(text) if text.parse::<i64>().is_ok() => "int4",
        Some(text) if text.parse::<f64>().is_ok() => "numeric",
        Some(text)
            if text.eq_ignore_ascii_case("true")
                || text.eq_ignore_ascii_case("false")
                || matches!(text, "t" | "f") =>
        {
            "bool"
        }
        Some(_) => "text",
    }
}

/// Split a declared parameter list.
///
/// Commas inside parentheses do not separate parameters, so
/// `a NUMERIC(10, 2), b TEXT` is two parameters and not three.
#[must_use]
pub fn parse_parameters(declaration: &str) -> Vec<Parameter> {
    split_top_level(declaration)
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .map(|part| parse_one(&part))
        .collect()
}

/// Split on commas that are not inside parentheses.
fn split_top_level(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;

    for c in text.chars() {
        match c {
            '(' => {
                depth += 1;
                current.push(c);
            }
            ')' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    parts.push(current);
    parts
}

/// Read one parameter: `[mode] name type`.
///
/// A parameter may also be written as just a type, with no name. It is kept
/// with an empty name; nothing can refer to it, which is what PostgreSQL also
/// gives you.
fn parse_one(declaration: &str) -> Parameter {
    let mut words = declaration.split_whitespace().peekable();

    let mode = match words.peek().map(|w| w.to_uppercase()) {
        Some(word) if word == "OUT" => {
            words.next();
            Mode::Out
        }
        Some(word) if word == "INOUT" => {
            words.next();
            Mode::InOut
        }
        Some(word) if word == "IN" => {
            words.next();
            Mode::In
        }
        _ => Mode::In,
    };

    let rest: Vec<&str> = words.collect();
    // One word is a bare type; two or more is a name followed by its type.
    let (name, sql_type) = match rest.split_first() {
        None => (String::new(), String::new()),
        Some((only, [])) => (String::new(), (*only).to_uppercase()),
        Some((name, tail)) => ((*name).to_lowercase(), tail.join(" ").to_uppercase()),
    };

    Parameter {
        name,
        sql_type,
        mode,
    }
}

/// How many parameters a caller passes.
#[must_use]
pub fn input_arity(parameters: &[Parameter]) -> usize {
    parameters.iter().filter(|p| p.mode.is_input()).count()
}

/// The parameters a caller passes, in order.
#[must_use]
pub fn inputs(parameters: &[Parameter]) -> Vec<&Parameter> {
    parameters.iter().filter(|p| p.mode.is_input()).collect()
}

/// The parameters the function returns, in order.
#[must_use]
pub fn outputs(parameters: &[Parameter]) -> Vec<&Parameter> {
    parameters.iter().filter(|p| p.mode.is_output()).collect()
}

/// The input types, for the catalog key.
///
/// Two functions of one name and arity are different functions only if this
/// differs. Canonical names rather than a coarse class, so `f(int4)` and
/// `f(int8)` are two entries and not one overwriting the other.
#[must_use]
pub fn signature(parameters: &[Parameter]) -> String {
    inputs(parameters)
        .iter()
        .map(|p| normalize(&p.sql_type))
        .collect::<Vec<_>>()
        .join("_")
}

/// Render parameters for the catalog.
#[must_use]
pub fn encode(parameters: &[Parameter]) -> String {
    parameters
        .iter()
        .map(|p| {
            format!(
                "{}{FIELD_SEPARATOR}{}{FIELD_SEPARATOR}{}",
                p.name,
                p.sql_type,
                p.mode.as_str()
            )
        })
        .collect::<Vec<_>>()
        .join(&PARAMETER_SEPARATOR.to_string())
}

/// Read parameters back from the catalog.
#[must_use]
pub fn decode(stored: &str) -> Vec<Parameter> {
    if stored.is_empty() {
        return Vec::new();
    }
    stored
        .split(PARAMETER_SEPARATOR)
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut fields = part.split(FIELD_SEPARATOR);
            Parameter {
                name: fields.next().unwrap_or_default().to_string(),
                sql_type: fields.next().unwrap_or_default().to_string(),
                mode: Mode::from_str(fields.next().unwrap_or("i")),
            }
        })
        .collect()
}

/// How well a candidate fits, for breaking a tie.
struct Fit {
    index: usize,
    /// Arguments whose type is exactly the parameter's.
    exact: usize,
    /// Parameters that are their category's preferred type.
    preferred: usize,
}

/// Choose which of several same-named functions a call means.
///
/// Follows PostgreSQL's order: keep the candidates every argument can be
/// converted to, prefer the one matching most arguments exactly, then the one
/// whose parameters are their categories' preferred types. Anything still tied
/// is ambiguous.
///
/// Returns the index of the chosen candidate.
///
/// # Errors
/// Returns an error when no candidate accepts the arguments, or when more than
/// one does and nothing distinguishes them — the same two answers PostgreSQL
/// gives, rather than silently picking one.
pub fn resolve(
    name: &str,
    candidates: &[Vec<Parameter>],
    argument_types: &[&str],
) -> ProtocolResult<usize> {
    let viable: Vec<Fit> = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, parameters)| {
            let wanted = inputs(parameters);
            if wanted.len() != argument_types.len() {
                return None;
            }
            let declared: Vec<String> = wanted.iter().map(|p| normalize(&p.sql_type)).collect();
            if !declared
                .iter()
                .zip(argument_types)
                .all(|(wanted, given)| converts_to(given, wanted))
            {
                return None;
            }
            Some(Fit {
                index,
                exact: declared
                    .iter()
                    .zip(argument_types)
                    .filter(|(wanted, given)| **given == **wanted)
                    .count(),
                preferred: declared.iter().filter(|t| is_preferred(t)).count(),
            })
        })
        .collect();

    if viable.is_empty() {
        return Err(ProtocolError::PostgresError(format!(
            "function {name}({}) does not exist",
            argument_types.join(", ")
        )));
    }

    // Most exact matches wins.
    let best_exact = viable.iter().map(|f| f.exact).max().unwrap_or(0);
    let mut shortlist: Vec<&Fit> = viable.iter().filter(|f| f.exact == best_exact).collect();

    // Then PostgreSQL's rule for untyped literals: at a position holding one,
    // if any candidate takes a string there, only those candidates stay. It is
    // why `f('x')` picks `f(text)` over `f(integer)` rather than being
    // ambiguous — a quoted literal reads as text unless something says
    // otherwise.
    let unknown_at: Vec<usize> = argument_types
        .iter()
        .enumerate()
        .filter(|(_, given)| **given == UNKNOWN)
        .map(|(index, _)| index)
        .collect();

    if !unknown_at.is_empty() && shortlist.len() > 1 {
        let takes_string = |fit: &&Fit| {
            let wanted = inputs(&candidates[fit.index]);
            unknown_at.iter().all(|position| {
                wanted
                    .get(*position)
                    .is_some_and(|p| category(&normalize(&p.sql_type)) == Category::String)
            })
        };
        if shortlist.iter().any(takes_string) {
            shortlist.retain(takes_string);
        }
    }

    // Finally the category's preferred type.
    let best_preferred = shortlist.iter().map(|f| f.preferred).max().unwrap_or(0);
    let finalists: Vec<&&Fit> = shortlist
        .iter()
        .filter(|f| f.preferred == best_preferred)
        .collect();

    match finalists.as_slice() {
        [only] => Ok(only.index),
        _ => Err(ProtocolError::PostgresError(format!(
            "function {name}({}) is not unique",
            argument_types.join(", ")
        ))),
    }
}

/// Check an argument against the type its parameter declares.
///
/// Only a mismatch PostgreSQL would also refuse is refused: text that is not a
/// number, passed where a number is wanted. A numeric-looking literal is
/// accepted, because an unadorned literal in SQL has no type until it is
/// assigned one.
///
/// # Errors
/// Returns `invalid_text_representation` when the value cannot be the declared
/// type.
pub fn check_argument(parameter: &Parameter, value: Option<&str>) -> ProtocolResult<()> {
    let canonical = normalize(&parameter.sql_type);
    let (Some(text), Category::Numeric) = (value, category(&canonical)) else {
        return Ok(());
    };
    if text.parse::<f64>().is_ok() {
        return Ok(());
    }
    Err(ProtocolError::SqlState {
        code: "22P02",
        message: format!(
            "invalid input syntax for type {}: \"{text}\"",
            parameter.sql_type.to_lowercase()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_type_with_a_comma_is_one_parameter() {
        // Splitting on every comma made this two parameters, the second of
        // them named `2)`.
        let parsed = parse_parameters("a NUMERIC(10, 2), b TEXT");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "a");
        assert_eq!(parsed[0].sql_type, "NUMERIC(10, 2)");
        assert_eq!(parsed[1].name, "b");
    }

    #[test]
    fn modes_are_read_and_default_to_in() {
        let parsed = parse_parameters("a INTEGER, OUT b INTEGER, INOUT c TEXT, IN d TEXT");
        assert_eq!(parsed[0].mode, Mode::In);
        assert_eq!(parsed[1].mode, Mode::Out);
        assert_eq!(parsed[2].mode, Mode::InOut);
        assert_eq!(parsed[3].mode, Mode::In);
    }

    #[test]
    fn only_input_parameters_are_counted_for_a_call() {
        let parsed = parse_parameters("a INTEGER, OUT b INTEGER, INOUT c INTEGER");
        assert_eq!(input_arity(&parsed), 2);
        assert_eq!(outputs(&parsed).len(), 2);
    }

    #[test]
    fn a_bare_type_has_no_name() {
        let parsed = parse_parameters("INTEGER");
        assert_eq!(parsed[0].name, "");
        assert_eq!(parsed[0].sql_type, "INTEGER");
    }

    #[test]
    fn an_empty_list_is_no_parameters() {
        assert!(parse_parameters("").is_empty());
        assert!(parse_parameters("   ").is_empty());
    }

    #[test]
    fn encoding_survives_a_type_containing_a_comma() {
        let parsed = parse_parameters("a NUMERIC(10, 2), OUT b TEXT");
        let restored = decode(&encode(&parsed));
        assert_eq!(restored, parsed);
    }

    #[test]
    fn a_type_is_reduced_to_its_canonical_name() {
        assert_eq!(normalize("INTEGER"), "int4");
        assert_eq!(normalize("INT"), "int4");
        assert_eq!(normalize("BIGINT"), "int8");
        assert_eq!(normalize("NUMERIC(10,2)"), "numeric");
        assert_eq!(normalize("VARCHAR(20)"), "varchar");
        assert_eq!(normalize("TEXT"), "text");
        assert_eq!(normalize(""), UNKNOWN);
    }

    #[test]
    fn an_array_carries_its_element_type() {
        assert_eq!(normalize("INTEGER[]"), "_int4");
        assert_eq!(normalize("TEXT ARRAY"), "_text");
        assert_eq!(normalize("BIGINT[]"), "_int8");
        assert_eq!(category("_int4"), Category::Array);

        // Two arrays of different elements are two types, so two overloads of
        // one name can take them.
        assert_ne!(normalize("INTEGER[]"), normalize("TEXT[]"));
        assert!(!converts_to("_int4", "_text"));
        assert!(converts_to("_int4", "_int4"));
        // Widening the element would mean rebuilding every entry.
        assert!(!converts_to("_int4", "_int8"));
        // An array never satisfies a scalar parameter, or a call would pass a
        // list where a value was wanted.
        assert!(!converts_to("_int4", "text"));
        assert!(!converts_to("int4", "_int4"));
    }

    #[test]
    fn arrays_of_different_elements_are_different_signatures() {
        assert_ne!(
            signature(&parse_parameters("a INTEGER[]")),
            signature(&parse_parameters("a TEXT[]"))
        );
    }

    #[test]
    fn conversion_widens_but_does_not_narrow() {
        assert!(converts_to("int4", "int8"));
        assert!(!converts_to("int8", "int4"));
        assert!(converts_to("int4", "numeric"));
        assert!(converts_to("varchar", "text"));
        assert!(!converts_to("text", "varchar"));
        // Categories do not mix, however few candidates remain.
        assert!(!converts_to("int4", "text"));
        assert!(!converts_to("bool", "int4"));
        // An untyped literal fits anything.
        assert!(converts_to(UNKNOWN, "int4"));
        assert!(converts_to(UNKNOWN, "text"));
    }

    #[test]
    fn an_arguments_type_comes_from_how_it_was_written() {
        assert_eq!(argument_type("42", Some("42")), "int4");
        // Too large for int4, as PostgreSQL also promotes it.
        assert_eq!(argument_type("5000000000", Some("5000000000")), "int8");
        assert_eq!(argument_type("3.14", Some("3.14")), "numeric");
        assert_eq!(argument_type("TRUE", Some("t")), "bool");
        // A quoted literal has no type until context gives it one.
        assert_eq!(argument_type("'5'", Some("5")), UNKNOWN);
        assert_eq!(argument_type("NULL", None), UNKNOWN);
    }

    #[test]
    fn int4_and_int8_are_told_apart() {
        // The whole point: these look identical once evaluated to text, and
        // resolving on the value alone could not choose between them.
        let small = parse_parameters("a INTEGER");
        let large = parse_parameters("a BIGINT");
        let candidates = vec![small, large];

        assert_eq!(resolve("f", &candidates, &["int4"]).expect("picks"), 0);
        assert_eq!(resolve("f", &candidates, &["int8"]).expect("picks"), 1);
    }

    #[test]
    fn varchar_and_text_are_told_apart() {
        let candidates = vec![parse_parameters("a VARCHAR"), parse_parameters("a TEXT")];
        assert_eq!(resolve("f", &candidates, &["varchar"]).expect("picks"), 0);
        assert_eq!(resolve("f", &candidates, &["text"]).expect("picks"), 1);
    }

    #[test]
    fn a_widening_call_reaches_the_only_candidate_that_fits() {
        // int4 does not fit int2, so only the int8 form is viable.
        let candidates = vec![parse_parameters("a SMALLINT"), parse_parameters("a BIGINT")];
        assert_eq!(resolve("f", &candidates, &["int4"]).expect("picks"), 1);
    }

    #[test]
    fn an_untyped_literal_prefers_the_preferred_type() {
        // `f('x')` against f(varchar) and f(text): both accept an unknown, and
        // PostgreSQL breaks the tie towards text.
        let candidates = vec![parse_parameters("a VARCHAR"), parse_parameters("a TEXT")];
        assert_eq!(resolve("f", &candidates, &[UNKNOWN]).expect("picks"), 1);

        // Among the integers the preferred type is int4.
        let integers = vec![parse_parameters("a BIGINT"), parse_parameters("a INTEGER")];
        assert_eq!(resolve("f", &integers, &[UNKNOWN]).expect("picks"), 1);
    }

    #[test]
    fn an_exact_match_beats_a_conversion() {
        let candidates = vec![parse_parameters("a NUMERIC"), parse_parameters("a INTEGER")];
        // int4 converts to numeric, but matches int4 exactly.
        assert_eq!(resolve("f", &candidates, &["int4"]).expect("picks"), 1);
    }

    #[test]
    fn a_call_picks_the_overload_whose_types_fit() {
        let candidates = vec![parse_parameters("a INTEGER"), parse_parameters("a TEXT")];
        assert_eq!(resolve("f", &candidates, &["int4"]).expect("picks"), 0);
        assert_eq!(resolve("f", &candidates, &["text"]).expect("picks"), 1);
    }

    #[test]
    fn arity_still_separates_overloads() {
        let candidates = vec![
            parse_parameters("a INTEGER"),
            parse_parameters("a INTEGER, b INTEGER"),
        ];
        assert_eq!(resolve("f", &candidates, &["int4"]).expect("picks"), 0);
        assert_eq!(
            resolve("f", &candidates, &["int4", "int4"]).expect("picks"),
            1
        );
    }

    #[test]
    fn an_untyped_literal_reads_as_text_when_a_candidate_takes_one() {
        // PostgreSQL resolves an unknown literal towards the string category,
        // so this is not ambiguous even though both candidates accept it.
        let candidates = vec![parse_parameters("a INTEGER"), parse_parameters("a TEXT")];
        assert_eq!(resolve("f", &candidates, &[UNKNOWN]).expect("picks"), 1);
    }

    #[test]
    fn an_ambiguous_call_is_refused_rather_than_guessed() {
        // No candidate takes a string, and both are their category's preferred
        // type, so nothing chooses between them.
        let candidates = vec![parse_parameters("a INTEGER"), parse_parameters("a BOOLEAN")];
        let failure = resolve("f", &candidates, &[UNKNOWN]).expect_err("ambiguous");
        assert!(failure.to_string().contains("not unique"));
    }

    #[test]
    fn no_candidate_is_an_error_naming_the_types() {
        let candidates = vec![parse_parameters("a INTEGER")];
        let failure =
            resolve("f", &candidates, &["int4", "int4"]).expect_err("no candidate takes two");
        assert!(failure.to_string().contains("does not exist"));
    }

    #[test]
    fn text_where_a_number_is_wanted_is_refused() {
        let parameter = &parse_parameters("a INTEGER")[0];
        assert!(check_argument(parameter, Some("42")).is_ok());
        assert!(check_argument(parameter, Some("-1.5")).is_ok());
        assert!(check_argument(parameter, None).is_ok());

        let failure = check_argument(parameter, Some("ada")).expect_err("refused");
        assert!(failure.to_string().contains("invalid input syntax"));
    }

    #[test]
    fn a_text_parameter_accepts_anything() {
        let parameter = &parse_parameters("a TEXT")[0];
        assert!(check_argument(parameter, Some("ada")).is_ok());
        assert!(check_argument(parameter, Some("42")).is_ok());
    }

    #[test]
    fn a_signature_tells_two_same_arity_functions_apart() {
        assert_ne!(
            signature(&parse_parameters("a INTEGER")),
            signature(&parse_parameters("a TEXT"))
        );
        // And two numeric widths are two functions, not one.
        assert_ne!(
            signature(&parse_parameters("a INTEGER")),
            signature(&parse_parameters("a BIGINT"))
        );
        // Output parameters are not part of what a caller passes, so they do
        // not belong in the key.
        assert_eq!(
            signature(&parse_parameters("a INTEGER, OUT b TEXT")),
            signature(&parse_parameters("a INTEGER"))
        );
    }
}
