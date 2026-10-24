//! Lowers PostgreSQL's own parse tree (`libpg_query`, PostgreSQL 18) into Orbit's [`Statement`] AST.
//!
//! `libpg_query` is the grammar of record: it decides what is valid SQL and with what error. This
//! module only translates its tree. A construct Orbit's AST cannot express yet is reported as
//! [`Lowered::Unsupported`], and the caller falls back to the hand-rolled parser for that
//! statement, so coverage grows one node type at a time without ever losing a statement.

use super::super::ast::*;
use super::super::types::{SqlType, SqlValue};
use pg_query::protobuf as pb;
use pg_query::protobuf::a_const::Val;
use pg_query::NodeEnum;
use std::collections::HashMap;

/// The outcome of lowering one SQL string.
#[derive(Debug)]
pub enum Lowered {
    /// Every statement was translated.
    Statements(Vec<Statement>),
    /// PostgreSQL's grammar rejected the text; the message is PostgreSQL's.
    SyntaxError(String),
    /// Valid SQL that uses something the AST cannot express yet; the reason names it.
    Unsupported(String),
}

/// Why a node could not be lowered.
#[derive(Debug)]
struct Unsupported(String);

type L<T> = Result<T, Unsupported>;

fn unsupported<T>(what: impl Into<String>) -> L<T> {
    Err(Unsupported(what.into()))
}

/// Parse `sql` with PostgreSQL's grammar and translate it.
pub fn lower(sql: &str) -> Lowered {
    let parsed = match pg_query::parse(sql, pg_query::ParserOptions::DEFAULT) {
        Ok(parsed) => parsed,
        Err(e) => return Lowered::SyntaxError(e.to_string()),
    };
    let mut out = Vec::with_capacity(parsed.protobuf.stmts.len());
    for raw in &parsed.protobuf.stmts {
        let Some(node) = raw.stmt.as_deref() else {
            continue;
        };
        match statement(node) {
            Ok(s) => out.push(s),
            Err(Unsupported(why)) => return Lowered::Unsupported(why),
        }
    }
    Lowered::Statements(out)
}

fn kind(node: &pb::Node) -> L<&NodeEnum> {
    node.node
        .as_ref()
        .ok_or_else(|| Unsupported("empty node".into()))
}

fn statement(node: &pb::Node) -> L<Statement> {
    Ok(match kind(node)? {
        NodeEnum::SelectStmt(s) => Statement::Select(Box::new(Cx::default().select(s)?)),
        NodeEnum::InsertStmt(s) => Statement::Insert(insert(s)?),
        NodeEnum::UpdateStmt(s) => Statement::Update(update(s)?),
        NodeEnum::DeleteStmt(s) => Statement::Delete(delete(s)?),
        NodeEnum::TransactionStmt(s) => transaction(s)?,
        NodeEnum::CreateStmt(s) => Statement::CreateTable(create_table(s)?),
        NodeEnum::CreateTableAsStmt(s) => Statement::CreateTable(create_table_as(s)?),
        NodeEnum::IndexStmt(s) => Statement::CreateIndex(create_index(s)?),
        NodeEnum::ViewStmt(s) => Statement::CreateView(create_view(s)?),
        NodeEnum::DropStmt(s) => drop_object(s)?,
        NodeEnum::TruncateStmt(s) => Statement::Truncate(truncate(s)?),
        NodeEnum::AlterTableStmt(s) => Statement::AlterTable(alter_table(s)?),
        NodeEnum::RenameStmt(s) => rename(s)?,
        other => return unsupported(format!("statement {}", node_name(other))),
    })
}

/// A node's variant name, for "unsupported" messages.
fn node_name(n: &NodeEnum) -> String {
    let text = format!("{n:?}");
    text.split(['(', ' ', '{'])
        .next()
        .unwrap_or("node")
        .to_string()
}

// ---- names -----------------------------------------------------------------------------------

fn string_of(node: &pb::Node) -> Option<&str> {
    match node.node.as_ref()? {
        NodeEnum::String(s) => Some(s.sval.as_str()),
        _ => None,
    }
}

fn names(list: &[pb::Node]) -> L<Vec<String>> {
    list.iter()
        .map(|n| {
            string_of(n)
                .map(str::to_string)
                .ok_or_else(|| Unsupported("a name that is not a plain identifier".into()))
        })
        .collect()
}

fn table_name(rv: &pb::RangeVar) -> TableName {
    TableName {
        schema: (!rv.schemaname.is_empty()).then(|| rv.schemaname.clone()),
        name: rv.relname.clone(),
    }
}

fn opt_table(rv: &Option<pb::RangeVar>) -> L<TableName> {
    rv.as_ref()
        .map(table_name)
        .ok_or_else(|| Unsupported("statement without a relation".into()))
}

// ---- types -----------------------------------------------------------------------------------

fn sql_type(tn: &pb::TypeName) -> L<SqlType> {
    let parts = names(&tn.names)?;
    let name = parts.last().map_or("", String::as_str).to_ascii_lowercase();
    let typmods: Vec<i64> = tn
        .typmods
        .iter()
        .filter_map(|m| match m.node.as_ref()? {
            NodeEnum::AConst(c) => match c.val.as_ref()? {
                Val::Ival(i) => Some(i64::from(i.ival)),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let len = typmods.first().and_then(|n| u32::try_from(*n).ok());
    let base = match name.as_str() {
        "int2" | "smallint" => SqlType::SmallInt,
        "int4" | "int" | "integer" => SqlType::Integer,
        "int8" | "bigint" => SqlType::BigInt,
        "float4" | "real" => SqlType::Real,
        "float8" => SqlType::DoublePrecision,
        "numeric" | "decimal" => SqlType::Numeric {
            precision: typmods.first().and_then(|n| u8::try_from(*n).ok()),
            scale: typmods.get(1).and_then(|n| u8::try_from(*n).ok()),
        },
        "bool" | "boolean" => SqlType::Boolean,
        "text" => SqlType::Text,
        "varchar" => SqlType::Varchar(len),
        "bpchar" | "char" => SqlType::Char(len),
        "date" => SqlType::Date,
        "time" => SqlType::Time {
            with_timezone: false,
        },
        "timetz" => SqlType::Time {
            with_timezone: true,
        },
        "timestamp" => SqlType::Timestamp {
            with_timezone: false,
        },
        "timestamptz" => SqlType::Timestamp {
            with_timezone: true,
        },
        "interval" => SqlType::Interval,
        "uuid" => SqlType::Uuid,
        "json" => SqlType::Json,
        "jsonb" => SqlType::Jsonb,
        "bytea" => SqlType::Bytea,
        "money" => SqlType::Money,
        "vector" => SqlType::Vector { dimensions: len },
        "halfvec" => SqlType::HalfVec { dimensions: len },
        "sparsevec" => SqlType::SparseVec { dimensions: len },
        other => SqlType::Custom {
            type_name: other.to_string(),
        },
    };
    Ok(if tn.array_bounds.is_empty() {
        base
    } else {
        tn.array_bounds.iter().fold(base, |element, _| SqlType::Array {
            element_type: Box::new(element),
            dimensions: None,
        })
    })
}

// ---- expressions -----------------------------------------------------------------------------

/// Lowering context: the `WINDOW name AS (..)` definitions of the enclosing query.
#[derive(Default)]
struct Cx {
    windows: HashMap<String, pb::WindowDef>,
}

fn literal(c: &pb::AConst) -> L<Expression> {
    if c.isnull {
        return Ok(Expression::Literal(SqlValue::Null));
    }
    let number = |text: &str| -> Expression {
        match super::super::durable::value::numeric_literal(text) {
            Some(v) => Expression::Literal(v),
            None => Expression::Literal(SqlValue::Text(text.to_string())),
        }
    };
    Ok(match c.val.as_ref() {
        Some(Val::Ival(i)) => {
            let n = i64::from(i.ival);
            if n < 0 {
                // the grammar folds `-5` into one constant; Orbit keeps the unary minus
                Expression::Unary {
                    operator: UnaryOperator::Minus,
                    operand: Box::new(number(&(-n).to_string())),
                }
            } else {
                number(&n.to_string())
            }
        }
        Some(Val::Fval(f)) => match f.fval.strip_prefix('-') {
            Some(abs) => Expression::Unary {
                operator: UnaryOperator::Minus,
                operand: Box::new(number(abs)),
            },
            None => number(&f.fval),
        },
        Some(Val::Sval(s)) => Expression::Literal(SqlValue::Text(s.sval.clone())),
        Some(Val::Boolval(b)) => Expression::Literal(SqlValue::Boolean(b.boolval)),
        Some(Val::Bsval(_)) => return unsupported("bit string literal"),
        None => Expression::Literal(SqlValue::Null),
    })
}

fn binary_operator(op: &str) -> L<BinaryOperator> {
    use BinaryOperator as B;
    Ok(match op {
        "+" => B::Plus,
        "-" => B::Minus,
        "*" => B::Multiply,
        "/" => B::Divide,
        "%" => B::Modulo,
        "^" => B::Power,
        "||" => B::Concat,
        "=" => B::Equal,
        "<>" | "!=" => B::NotEqual,
        "<" => B::LessThan,
        "<=" => B::LessThanOrEqual,
        ">" => B::GreaterThan,
        ">=" => B::GreaterThanOrEqual,
        "~" => B::RegexMatch,
        "~*" => B::RegexMatchCaseInsensitive,
        "!~" => B::RegexNotMatch,
        "!~*" => B::RegexNotMatchCaseInsensitive,
        "->" => B::JsonExtract,
        "->>" => B::JsonExtractText,
        "#>" => B::JsonPathExtract,
        "#>>" => B::JsonPathExtractText,
        "@>" => B::RangeContains,
        "<@" => B::RangeContainedBy,
        "&&" => B::RangeOverlaps,
        "?" => B::JsonExists,
        "?|" => B::JsonExistsAny,
        "?&" => B::JsonExistsAll,
        "@?" => B::JsonPathExists,
        "@@" => B::JsonPathMatch,
        "#-" => B::JsonDeletePath,
        "&" => B::BitwiseAnd,
        "|" => B::BitwiseOr,
        "#" => B::BitwiseXor,
        "<<" => B::RangeStrictlyLeft,
        ">>" => B::RangeStrictlyRight,
        "-|-" => B::RangeAdjacent,
        "&<" => B::RangeNotExtendRight,
        "&>" => B::RangeNotExtendLeft,
        "<->" => B::VectorDistance,
        "<#>" => B::VectorInnerProduct,
        "<=>" => B::VectorCosineDistance,
        "^@" => B::StartsWith,
        other => return unsupported(format!("operator {other}")),
    })
}

fn unary_operator(op: &str) -> L<UnaryOperator> {
    Ok(match op {
        "-" => UnaryOperator::Minus,
        "+" => UnaryOperator::Plus,
        "~" => UnaryOperator::BitwiseNot,
        "@" => UnaryOperator::AbsoluteValue,
        "|/" => UnaryOperator::SquareRoot,
        "||/" => UnaryOperator::CubeRoot,
        other => return unsupported(format!("prefix operator {other}")),
    })
}

fn bx(e: Expression) -> Box<Expression> {
    Box::new(e)
}

impl Cx {
    fn expr(&self, node: &pb::Node) -> L<Expression> {
        Ok(match kind(node)? {
            NodeEnum::AConst(c) => literal(c)?,
            NodeEnum::ColumnRef(c) => column(c)?,
            NodeEnum::ParamRef(p) => Expression::Parameter(p.number as u32),
            NodeEnum::AExpr(e) => self.a_expr(e)?,
            NodeEnum::BoolExpr(b) => self.bool_expr(b)?,
            NodeEnum::NullTest(t) => {
                let arg = self.boxed(&t.arg)?;
                let negated = t.nulltesttype == pb::NullTestType::IsNotNull as i32;
                Expression::Binary {
                    left: arg,
                    operator: if negated {
                        BinaryOperator::IsNot
                    } else {
                        BinaryOperator::Is
                    },
                    right: bx(Expression::Literal(SqlValue::Null)),
                }
            }
            NodeEnum::BooleanTest(t) => {
                use pb::BoolTestType as T;
                let arg = self.boxed(&t.arg)?;
                let (negated, value) = match T::try_from(t.booltesttype) {
                    Ok(T::IsTrue) => (false, Some(true)),
                    Ok(T::IsNotTrue) => (true, Some(true)),
                    Ok(T::IsFalse) => (false, Some(false)),
                    Ok(T::IsNotFalse) => (true, Some(false)),
                    Ok(T::IsUnknown) => (false, None),
                    Ok(T::IsNotUnknown) => (true, None),
                    _ => return unsupported("boolean test"),
                };
                Expression::Binary {
                    left: arg,
                    operator: if negated {
                        BinaryOperator::IsNot
                    } else {
                        BinaryOperator::Is
                    },
                    right: bx(Expression::Literal(match value {
                        Some(b) => SqlValue::Boolean(b),
                        None => SqlValue::Null,
                    })),
                }
            }
            NodeEnum::TypeCast(t) => {
                let tn = t
                    .type_name
                    .as_ref()
                    .ok_or_else(|| Unsupported("cast without a type".into()))?;
                Expression::Cast {
                    expr: self.boxed(&t.arg)?,
                    target_type: sql_type(tn)?,
                }
            }
            NodeEnum::FuncCall(f) => self.func_call(f)?,
            NodeEnum::CaseExpr(c) => Expression::Case(CaseExpression {
                operand: match &c.arg {
                    Some(a) => Some(bx(self.expr(a)?)),
                    None => None,
                },
                when_clauses: c
                    .args
                    .iter()
                    .map(|w| match kind(w)? {
                        NodeEnum::CaseWhen(w) => Ok(WhenClause {
                            condition: self.boxed(&w.expr)?,
                            result: self.boxed(&w.result)?,
                        }),
                        _ => unsupported("CASE branch"),
                    })
                    .collect::<L<_>>()?,
                else_clause: match &c.defresult {
                    Some(d) => Some(bx(self.expr(d)?)),
                    None => None,
                },
            }),
            NodeEnum::SubLink(s) => self.sublink(s)?,
            NodeEnum::AArrayExpr(a) => {
                Expression::Array(a.elements.iter().map(|e| self.expr(e)).collect::<L<_>>()?)
            }
            NodeEnum::RowExpr(r) => {
                Expression::Row(r.args.iter().map(|e| self.expr(e)).collect::<L<_>>()?)
            }
            NodeEnum::AIndirection(i) => self.indirection(i)?,
            NodeEnum::SqlvalueFunction(f) => sql_value_function(f)?,
            NodeEnum::CoalesceExpr(c) => self.plain_function("COALESCE", &c.args)?,
            NodeEnum::MinMaxExpr(m) => {
                let name = if m.op == pb::MinMaxOp::IsGreatest as i32 {
                    "GREATEST"
                } else {
                    "LEAST"
                };
                self.plain_function(name, &m.args)?
            }
            other => return unsupported(format!("expression {}", node_name(other))),
        })
    }

    fn boxed(&self, node: &Option<Box<pb::Node>>) -> L<Box<Expression>> {
        match node {
            Some(n) => Ok(bx(self.expr(n)?)),
            None => unsupported("missing operand"),
        }
    }

    fn plain_function(&self, name: &str, args: &[pb::Node]) -> L<Expression> {
        Ok(Expression::Function(Box::new(FunctionCall {
            name: FunctionName::Simple(name.to_string()),
            args: args.iter().map(|a| self.expr(a)).collect::<L<_>>()?,
            distinct: false,
            order_by: None,
            filter: None,
            within_group: None,
        })))
    }

    fn a_expr(&self, e: &pb::AExpr) -> L<Expression> {
        use pb::AExprKind as K;
        let op = e
            .name
            .last()
            .and_then(string_of)
            .ok_or_else(|| Unsupported("operator without a name".into()))?;
        let kind = K::try_from(e.kind).map_err(|_| Unsupported("operator kind".into()))?;
        let lhs = || self.boxed(&e.lexpr);
        let rhs = || self.boxed(&e.rexpr);
        Ok(match kind {
            K::AexprOp => match (&e.lexpr, &e.rexpr) {
                (None, Some(_)) => Expression::Unary {
                    operator: unary_operator(op)?,
                    operand: rhs()?,
                },
                _ => Expression::Binary {
                    left: lhs()?,
                    operator: binary_operator(op)?,
                    right: rhs()?,
                },
            },
            K::AexprOpAny | K::AexprOpAll => {
                let quantified = rhs()?;
                Expression::Binary {
                    left: lhs()?,
                    operator: binary_operator(op)?,
                    right: bx(if kind == K::AexprOpAny {
                        Expression::Any(quantified)
                    } else {
                        Expression::All(quantified)
                    }),
                }
            }
            K::AexprDistinct => Expression::Binary {
                left: lhs()?,
                operator: BinaryOperator::IsDistinctFrom,
                right: rhs()?,
            },
            K::AexprNotDistinct => Expression::Binary {
                left: lhs()?,
                operator: BinaryOperator::IsNotDistinctFrom,
                right: rhs()?,
            },
            K::AexprNullif => {
                let args = [lhs()?, rhs()?];
                Expression::Function(Box::new(FunctionCall {
                    name: FunctionName::Simple("NULLIF".to_string()),
                    args: args.into_iter().map(|b| *b).collect(),
                    distinct: false,
                    order_by: None,
                    filter: None,
                    within_group: None,
                }))
            }
            K::AexprIn => {
                let list = match e.rexpr.as_deref().and_then(|n| n.node.as_ref()) {
                    Some(NodeEnum::List(l)) => {
                        l.items.iter().map(|i| self.expr(i)).collect::<L<Vec<_>>>()?
                    }
                    _ => return unsupported("IN without a list"),
                };
                Expression::In {
                    expr: lhs()?,
                    list: InList::Expressions(list),
                    negated: op == "<>",
                }
            }
            K::AexprLike | K::AexprIlike => {
                let negated = op.starts_with('!');
                let insensitive = kind == K::AexprIlike;
                // `LIKE .. ESCAPE c` arrives as like_escape(pattern, c)
                let (pattern, escape) = match e.rexpr.as_deref().and_then(|n| n.node.as_ref()) {
                    Some(NodeEnum::FuncCall(f))
                        if f.funcname.last().and_then(string_of) == Some("like_escape")
                            && f.args.len() == 2 =>
                    {
                        (
                            bx(self.expr(&f.args[0])?),
                            Some(bx(self.expr(&f.args[1])?)),
                        )
                    }
                    _ => (rhs()?, None),
                };
                if escape.is_none() && !(insensitive && negated) {
                    Expression::Binary {
                        left: lhs()?,
                        operator: match (insensitive, negated) {
                            (false, false) => BinaryOperator::Like,
                            (false, true) => BinaryOperator::NotLike,
                            _ => BinaryOperator::ILike,
                        },
                        right: pattern,
                    }
                } else {
                    Expression::Like {
                        expr: lhs()?,
                        pattern,
                        escape,
                        case_insensitive: insensitive,
                        negated,
                    }
                }
            }
            K::AexprSimilar => {
                let negated = op.starts_with('!');
                // SIMILAR TO arrives as similar_to_escape(pattern [, escape])
                let pattern = match e.rexpr.as_deref().and_then(|n| n.node.as_ref()) {
                    Some(NodeEnum::FuncCall(f))
                        if f.funcname.last().and_then(string_of) == Some("similar_to_escape")
                            && f.args.len() == 1 =>
                    {
                        bx(self.expr(&f.args[0])?)
                    }
                    _ => return unsupported("SIMILAR TO with ESCAPE"),
                };
                Expression::Binary {
                    left: lhs()?,
                    operator: if negated {
                        BinaryOperator::NotSimilarTo
                    } else {
                        BinaryOperator::SimilarTo
                    },
                    right: pattern,
                }
            }
            K::AexprBetween | K::AexprNotBetween => {
                let bounds = match e.rexpr.as_deref().and_then(|n| n.node.as_ref()) {
                    Some(NodeEnum::List(l)) if l.items.len() == 2 => &l.items,
                    _ => return unsupported("BETWEEN bounds"),
                };
                Expression::Between {
                    expr: lhs()?,
                    low: bx(self.expr(&bounds[0])?),
                    high: bx(self.expr(&bounds[1])?),
                    negated: kind == K::AexprNotBetween,
                }
            }
            K::AexprBetweenSym | K::AexprNotBetweenSym | K::Undefined => {
                return unsupported("BETWEEN SYMMETRIC")
            }
        })
    }

    fn bool_expr(&self, b: &pb::BoolExpr) -> L<Expression> {
        use pb::BoolExprType as T;
        let fold = |op: BinaryOperator| -> L<Expression> {
            let mut parts = b.args.iter().map(|a| self.expr(a));
            let first = parts.next().ok_or_else(|| Unsupported("empty AND/OR".into()))??;
            parts.try_fold(first, |acc, next| {
                Ok(Expression::Binary {
                    left: bx(acc),
                    operator: op.clone(),
                    right: bx(next?),
                })
            })
        };
        match T::try_from(b.boolop) {
            Ok(T::AndExpr) => fold(BinaryOperator::And),
            Ok(T::OrExpr) => fold(BinaryOperator::Or),
            Ok(T::NotExpr) => {
                let inner = self.expr(
                    b.args
                        .first()
                        .ok_or_else(|| Unsupported("NOT without an operand".into()))?,
                )?;
                // NOT (x IN (..)) is how the grammar spells NOT IN over a subquery
                Ok(match inner {
                    Expression::In {
                        expr,
                        list,
                        negated,
                    } => Expression::In {
                        expr,
                        list,
                        negated: !negated,
                    },
                    other => Expression::Unary {
                        operator: UnaryOperator::Not,
                        operand: bx(other),
                    },
                })
            }
            _ => unsupported("boolean operator"),
        }
    }

    fn sublink(&self, s: &pb::SubLink) -> L<Expression> {
        use pb::SubLinkType as T;
        let subquery = || -> L<SelectStatement> {
            match s.subselect.as_deref().and_then(|n| n.node.as_ref()) {
                Some(NodeEnum::SelectStmt(sel)) => Cx::default().select(sel),
                _ => unsupported("subquery"),
            }
        };
        let operator = || -> L<BinaryOperator> {
            binary_operator(
                s.oper_name
                    .last()
                    .and_then(string_of)
                    .ok_or_else(|| Unsupported("quantified comparison without operator".into()))?,
            )
        };
        Ok(match T::try_from(s.sub_link_type) {
            Ok(T::ExistsSublink) => Expression::Exists(Box::new(subquery()?)),
            Ok(T::ExprSublink) => Expression::Subquery(Box::new(subquery()?)),
            Ok(T::AnySublink) if s.oper_name.is_empty() => Expression::In {
                expr: self.boxed(&s.testexpr)?,
                list: InList::Subquery(Box::new(subquery()?)),
                negated: false,
            },
            Ok(T::AnySublink) => Expression::Binary {
                left: self.boxed(&s.testexpr)?,
                operator: operator()?,
                right: bx(Expression::Any(bx(Expression::Subquery(Box::new(
                    subquery()?,
                ))))),
            },
            Ok(T::AllSublink) => Expression::Binary {
                left: self.boxed(&s.testexpr)?,
                operator: operator()?,
                right: bx(Expression::All(bx(Expression::Subquery(Box::new(
                    subquery()?,
                ))))),
            },
            Ok(T::ArraySublink) => Expression::Function(Box::new(FunctionCall {
                name: FunctionName::Simple("array_from_subquery".to_string()),
                args: vec![Expression::Subquery(Box::new(subquery()?))],
                distinct: false,
                order_by: None,
                filter: None,
                within_group: None,
            })),
            _ => return unsupported("this kind of subquery comparison"),
        })
    }

    fn indirection(&self, i: &pb::AIndirection) -> L<Expression> {
        let mut value = self.boxed(&i.arg)?;
        for step in &i.indirection {
            let NodeEnum::AIndices(ix) = kind(step)? else {
                return unsupported("field selection");
            };
            let bound = |n: &Option<Box<pb::Node>>| -> L<Option<Box<Expression>>> {
                match n {
                    Some(b) => Ok(Some(bx(self.expr(b)?))),
                    None => Ok(None),
                }
            };
            value = bx(if ix.is_slice {
                Expression::ArraySlice {
                    array: value,
                    start: bound(&ix.lidx)?,
                    end: bound(&ix.uidx)?,
                }
            } else {
                Expression::ArrayIndex {
                    array: value,
                    index: bound(&ix.uidx)?
                        .ok_or_else(|| Unsupported("array subscript".into()))?,
                }
            });
        }
        Ok(*value)
    }

    fn func_call(&self, f: &pb::FuncCall) -> L<Expression> {
        let parts = names(&f.funcname)?;
        let raw = parts
            .last()
            .ok_or_else(|| Unsupported("function without a name".into()))?
            .clone();
        let mut args: Vec<Expression> = if f.agg_star {
            vec![Expression::Column(ColumnRef {
                table: None,
                name: "*".to_string(),
            })]
        } else {
            f.args.iter().map(|a| self.expr(a)).collect::<L<_>>()?
        };
        // position(sub IN string) reaches the grammar as position(string, sub); Orbit takes (sub, string)
        if raw == "position" && args.len() == 2 {
            args.swap(0, 1);
        }
        if f.func_variadic {
            return unsupported("VARIADIC call");
        }
        let order: Vec<OrderByItem> = f
            .agg_order
            .iter()
            .map(|o| self.sort_item(o))
            .collect::<L<_>>()?;
        let call = FunctionCall {
            name: FunctionName::Simple(raw.clone()),
            args,
            distinct: f.agg_distinct,
            order_by: (!f.agg_within_group && !order.is_empty()).then(|| order.clone()),
            filter: match &f.agg_filter {
                Some(fl) => Some(self.boxed(&Some(fl.clone()))?),
                None => None,
            },
            within_group: (f.agg_within_group).then_some(order),
        };
        match &f.over {
            Some(over) => self.window(call, over),
            None => Ok(Expression::Function(Box::new(call))),
        }
    }

    fn window(&self, call: FunctionCall, over: &pb::WindowDef) -> L<Expression> {
        // OVER w: the named definition supplies whatever the reference does not override
        let base = if over.refname.is_empty() {
            None
        } else {
            Some(
                self.windows
                    .get(&over.refname)
                    .ok_or_else(|| Unsupported(format!("window {}", over.refname)))?,
            )
        };
        let def = match base {
            Some(b) if over.partition_clause.is_empty() && over.order_clause.is_empty() && over.frame_options & 1 == 0 => b,
            _ => over,
        };
        let partition_by = def
            .partition_clause
            .iter()
            .map(|p| self.expr(p))
            .collect::<L<_>>()?;
        let order_by = def
            .order_clause
            .iter()
            .map(|o| self.sort_item(o))
            .collect::<L<_>>()?;
        let frame = self.frame(def)?;
        let name = match &call.name {
            FunctionName::Simple(n) => n.to_ascii_lowercase(),
            FunctionName::Qualified { name, .. } => name.to_ascii_lowercase(),
        };
        let arg = |i: usize| call.args.get(i).cloned().map(bx);
        let need = |i: usize| arg(i).ok_or_else(|| Unsupported(format!("{name}() argument")));
        let function = match name.as_str() {
            "row_number" => WindowFunctionType::RowNumber,
            "rank" => WindowFunctionType::Rank,
            "dense_rank" => WindowFunctionType::DenseRank,
            "percent_rank" => WindowFunctionType::PercentRank,
            "cume_dist" => WindowFunctionType::CumeDist,
            "ntile" => WindowFunctionType::Ntile(need(0)?),
            "lag" => WindowFunctionType::Lag {
                expr: need(0)?,
                offset: arg(1),
                default: arg(2),
            },
            "lead" => WindowFunctionType::Lead {
                expr: need(0)?,
                offset: arg(1),
                default: arg(2),
            },
            "first_value" => WindowFunctionType::FirstValue(need(0)?),
            "last_value" => WindowFunctionType::LastValue(need(0)?),
            "nth_value" => WindowFunctionType::NthValue {
                expr: need(0)?,
                n: need(1)?,
            },
            _ => WindowFunctionType::Aggregate(Box::new(call)),
        };
        Ok(Expression::WindowFunction {
            function,
            partition_by,
            order_by,
            frame,
        })
    }

    fn frame(&self, def: &pb::WindowDef) -> L<Option<WindowFrame>> {
        const NONDEFAULT: i32 = 0x01;
        const RANGE: i32 = 0x02;
        const ROWS: i32 = 0x04;
        const GROUPS: i32 = 0x08;
        const BETWEEN: i32 = 0x10;
        const START_UNBOUNDED_PRECEDING: i32 = 0x20;
        const END_UNBOUNDED_FOLLOWING: i32 = 0x100;
        const START_CURRENT_ROW: i32 = 0x200;
        const END_CURRENT_ROW: i32 = 0x400;
        const START_OFFSET_PRECEDING: i32 = 0x800;
        const START_OFFSET_FOLLOWING: i32 = 0x1000;
        const END_OFFSET_PRECEDING: i32 = 0x2000;
        const END_OFFSET_FOLLOWING: i32 = 0x4000;
        const EXCLUDE_CURRENT_ROW: i32 = 0x8000;
        const EXCLUDE_GROUP: i32 = 0x10000;
        const EXCLUDE_TIES: i32 = 0x20000;
        let o = def.frame_options;
        if o & NONDEFAULT == 0 {
            return Ok(None);
        }
        let mode = if o & ROWS != 0 {
            WindowFrameMode::Rows
        } else if o & GROUPS != 0 {
            WindowFrameMode::Groups
        } else if o & RANGE != 0 {
            WindowFrameMode::Range
        } else {
            return unsupported("window frame mode");
        };
        let offset = |n: &Option<Box<pb::Node>>| self.boxed(n);
        let start_bound = if o & START_UNBOUNDED_PRECEDING != 0 {
            FrameBound::UnboundedPreceding
        } else if o & START_CURRENT_ROW != 0 {
            FrameBound::CurrentRow
        } else if o & START_OFFSET_PRECEDING != 0 {
            FrameBound::Preceding(offset(&def.start_offset)?)
        } else if o & START_OFFSET_FOLLOWING != 0 {
            FrameBound::Following(offset(&def.start_offset)?)
        } else {
            return unsupported("window frame start");
        };
        let end_bound = if o & BETWEEN == 0 {
            None
        } else if o & END_UNBOUNDED_FOLLOWING != 0 {
            Some(FrameBound::UnboundedFollowing)
        } else if o & END_CURRENT_ROW != 0 {
            Some(FrameBound::CurrentRow)
        } else if o & END_OFFSET_PRECEDING != 0 {
            Some(FrameBound::Preceding(offset(&def.end_offset)?))
        } else if o & END_OFFSET_FOLLOWING != 0 {
            Some(FrameBound::Following(offset(&def.end_offset)?))
        } else {
            return unsupported("window frame end");
        };
        let exclusion = if o & EXCLUDE_CURRENT_ROW != 0 {
            Some(WindowFrameExclusion::CurrentRow)
        } else if o & EXCLUDE_GROUP != 0 {
            Some(WindowFrameExclusion::Group)
        } else if o & EXCLUDE_TIES != 0 {
            Some(WindowFrameExclusion::Ties)
        } else {
            None
        };
        Ok(Some(WindowFrame {
            mode,
            start_bound,
            end_bound,
            exclusion,
        }))
    }

    fn sort_item(&self, node: &pb::Node) -> L<OrderByItem> {
        let NodeEnum::SortBy(s) = kind(node)? else {
            return unsupported("ORDER BY item");
        };
        if !s.use_op.is_empty() {
            return unsupported("ORDER BY ... USING");
        }
        use pb::{SortByDir as D, SortByNulls as N};
        Ok(OrderByItem {
            expression: self.expr(
                s.node
                    .as_deref()
                    .ok_or_else(|| Unsupported("empty ORDER BY".into()))?,
            )?,
            direction: match D::try_from(s.sortby_dir) {
                Ok(D::SortbyAsc) => Some(SortDirection::Ascending),
                Ok(D::SortbyDesc) => Some(SortDirection::Descending),
                _ => None,
            },
            nulls: match N::try_from(s.sortby_nulls) {
                Ok(N::SortbyNullsFirst) => Some(NullsOrder::First),
                Ok(N::SortbyNullsLast) => Some(NullsOrder::Last),
                _ => None,
            },
        })
    }
}

fn column(c: &pb::ColumnRef) -> L<Expression> {
    let mut parts = Vec::new();
    for f in &c.fields {
        match f.node.as_ref() {
            Some(NodeEnum::String(s)) => parts.push(s.sval.clone()),
            Some(NodeEnum::AStar(_)) => parts.push("*".to_string()),
            _ => return unsupported("column reference"),
        }
    }
    Ok(match parts.as_slice() {
        [name] => Expression::Column(ColumnRef {
            table: None,
            name: name.clone(),
        }),
        [table, name] | [_, table, name] => Expression::Column(ColumnRef {
            table: Some(table.clone()),
            name: name.clone(),
        }),
        _ => return unsupported("column reference"),
    })
}

fn sql_value_function(f: &pb::SqlValueFunction) -> L<Expression> {
    use pb::SqlValueFunctionOp as O;
    let precision = (f.typmod >= 0).then_some(f.typmod as u32);
    let session = |name: &str| {
        Expression::Column(ColumnRef {
            table: None,
            name: name.to_string(),
        })
    };
    Ok(
        match O::try_from(f.op).map_err(|_| Unsupported("SQL value function".into()))? {
            O::SvfopCurrentDate => Expression::CurrentDate,
            O::SvfopCurrentTime | O::SvfopCurrentTimeN => Expression::CurrentTime(precision),
            O::SvfopCurrentTimestamp | O::SvfopCurrentTimestampN => {
                Expression::CurrentTimestamp(precision)
            }
            O::SvfopLocaltime | O::SvfopLocaltimeN => Expression::LocalTime(precision),
            O::SvfopLocaltimestamp | O::SvfopLocaltimestampN => {
                Expression::LocalTimestamp(precision)
            }
            O::SvfopCurrentUser | O::SvfopCurrentRole => session("current_user"),
            O::SvfopUser => session("user"),
            O::SvfopSessionUser => session("session_user"),
            O::SvfopCurrentCatalog => session("current_catalog"),
            O::SvfopCurrentSchema => session("current_schema"),
            O::SqlvalueFunctionOpUndefined => return unsupported("SQL value function"),
        },
    )
}

// ---- SELECT ----------------------------------------------------------------------------------

impl Cx {
    fn select(&self, s: &pb::SelectStmt) -> L<SelectStatement> {
        use pb::SetOperation as S;
        let mut cx = Cx {
            windows: self.windows.clone(),
        };
        for w in &s.window_clause {
            if let Some(NodeEnum::WindowDef(d)) = w.node.as_ref() {
                cx.windows.insert(d.name.clone(), (**d).clone());
            }
        }
        let cx = &cx;
        if !matches!(S::try_from(s.op), Ok(S::SetopNone)) {
            return cx.set_operation(s);
        }
        if s.into_clause.is_some() {
            return unsupported("SELECT INTO");
        }
        let with = match &s.with_clause {
            Some(w) => Some(cx.with_clause(w)?),
            None => None,
        };
        let from_clause = if !s.values_lists.is_empty() {
            Some(FromClause::Values {
                values: s
                    .values_lists
                    .iter()
                    .map(|row| match kind(row)? {
                        NodeEnum::List(l) => l.items.iter().map(|e| cx.expr(e)).collect(),
                        _ => unsupported("VALUES row"),
                    })
                    .collect::<L<_>>()?,
                alias: None,
            })
        } else {
            cx.from_list(&s.from_clause)?
        };
        let select_list = if !s.values_lists.is_empty() {
            vec![SelectItem::Wildcard]
        } else {
            s.target_list
                .iter()
                .map(|t| cx.target(t))
                .collect::<L<_>>()?
        };
        let distinct = if s.distinct_clause.is_empty() {
            None
        } else {
            let on: Vec<Expression> = s
                .distinct_clause
                .iter()
                .filter(|n| n.node.is_some())
                .map(|n| cx.expr(n))
                .collect::<L<_>>()?;
            Some(if on.is_empty() {
                DistinctClause::Distinct
            } else {
                DistinctClause::DistinctOn(on)
            })
        };
        let group_by = if s.group_clause.is_empty() {
            None
        } else {
            Some(
                s.group_clause
                    .iter()
                    .map(|g| cx.expr(g))
                    .collect::<L<Vec<_>>>()?,
            )
        };
        let order_by = if s.sort_clause.is_empty() {
            None
        } else {
            Some(
                s.sort_clause
                    .iter()
                    .map(|o| cx.sort_item(o))
                    .collect::<L<_>>()?,
            )
        };
        let (limit, offset) = cx.limit(s)?;
        Ok(SelectStatement {
            with,
            select_list,
            distinct,
            from_clause,
            where_clause: match &s.where_clause {
                Some(w) => Some(cx.expr(w)?),
                None => None,
            },
            group_by,
            having: match &s.having_clause {
                Some(h) => Some(cx.expr(h)?),
                None => None,
            },
            order_by,
            limit,
            offset,
            for_clause: cx.locking(&s.locking_clause)?,
            traverse: None,
            set_operation: None,
        })
    }

    fn set_operation(&self, s: &pb::SelectStmt) -> L<SelectStatement> {
        use pb::SetOperation as S;
        let (Some(l), Some(r)) = (s.larg.as_deref(), s.rarg.as_deref()) else {
            return unsupported("set operation operands");
        };
        let operator = match (S::try_from(s.op), s.all) {
            (Ok(S::SetopUnion), false) => SetOperator::Union,
            (Ok(S::SetopUnion), true) => SetOperator::UnionAll,
            (Ok(S::SetopIntersect), false) => SetOperator::Intersect,
            (Ok(S::SetopIntersect), true) => SetOperator::IntersectAll,
            (Ok(S::SetopExcept), false) => SetOperator::Except,
            (Ok(S::SetopExcept), true) => SetOperator::ExceptAll,
            _ => return unsupported("set operator"),
        };
        let mut head = self.select(l)?;
        let right = self.select(r)?;
        // A UNION B UNION C chains to the right: append B (and what follows) at the tail
        let mut tail = &mut head;
        while tail.set_operation.is_some() {
            tail = &mut tail.set_operation.as_mut().expect("checked").right;
        }
        tail.set_operation = Some(SetOperation {
            operator,
            right: Box::new(right),
        });
        // ORDER BY / LIMIT / WITH written after the last operand belong to the whole compound query
        if !s.sort_clause.is_empty() {
            head.order_by = Some(
                s.sort_clause
                    .iter()
                    .map(|o| self.sort_item(o))
                    .collect::<L<_>>()?,
            );
        }
        let (limit, offset) = self.limit(s)?;
        if limit.is_some() {
            head.limit = limit;
        }
        if offset.is_some() {
            head.offset = offset;
        }
        if let Some(w) = &s.with_clause {
            head.with = Some(self.with_clause(w)?);
        }
        Ok(head)
    }

    fn with_clause(&self, w: &pb::WithClause) -> L<WithClause> {
        let ctes = w
            .ctes
            .iter()
            .map(|c| match kind(c)? {
                NodeEnum::CommonTableExpr(cte) => {
                    let query = match cte.ctequery.as_deref().and_then(|n| n.node.as_ref()) {
                        Some(NodeEnum::SelectStmt(q)) => self.select(q)?,
                        _ => return unsupported("data-modifying CTE"),
                    };
                    Ok(CommonTableExpression {
                        name: cte.ctename.clone(),
                        columns: if cte.aliascolnames.is_empty() {
                            None
                        } else {
                            Some(names(&cte.aliascolnames)?)
                        },
                        query: Box::new(query),
                    })
                }
                _ => unsupported("WITH item"),
            })
            .collect::<L<_>>()?;
        Ok(WithClause {
            recursive: w.recursive,
            ctes,
        })
    }

    fn target(&self, node: &pb::Node) -> L<SelectItem> {
        let NodeEnum::ResTarget(t) = kind(node)? else {
            return unsupported("select item");
        };
        let val = t
            .val
            .as_deref()
            .ok_or_else(|| Unsupported("empty select item".into()))?;
        if let Some(NodeEnum::ColumnRef(c)) = val.node.as_ref() {
            if matches!(c.fields.last().and_then(|f| f.node.as_ref()), Some(NodeEnum::AStar(_))) {
                return Ok(match c.fields.len() {
                    1 => SelectItem::Wildcard,
                    _ => SelectItem::QualifiedWildcard {
                        qualifier: string_of(&c.fields[c.fields.len() - 2])
                            .unwrap_or_default()
                            .to_string(),
                    },
                });
            }
        }
        Ok(SelectItem::Expression {
            expr: self.expr(val)?,
            alias: (!t.name.is_empty()).then(|| t.name.clone()),
        })
    }

    fn from_list(&self, items: &[pb::Node]) -> L<Option<FromClause>> {
        let mut it = items.iter();
        let Some(first) = it.next() else {
            return Ok(None);
        };
        let mut left = self.from_item(first)?;
        for next in it {
            left = FromClause::Join {
                left: Box::new(left),
                join_type: JoinType::Cross,
                right: Box::new(self.from_item(next)?),
                condition: JoinCondition::Natural,
            };
        }
        Ok(Some(left))
    }

    fn alias(a: &Option<pb::Alias>) -> L<Option<TableAlias>> {
        match a {
            None => Ok(None),
            Some(a) => Ok(Some(TableAlias {
                name: a.aliasname.clone(),
                columns: if a.colnames.is_empty() {
                    None
                } else {
                    Some(names(&a.colnames)?)
                },
            })),
        }
    }

    fn from_item(&self, node: &pb::Node) -> L<FromClause> {
        Ok(match kind(node)? {
            NodeEnum::RangeVar(rv) => FromClause::Table {
                name: table_name(rv),
                alias: Self::alias(&rv.alias)?,
                time_travel: None,
            },
            NodeEnum::JoinExpr(j) => self.join(j)?,
            NodeEnum::RangeSubselect(r) => {
                let query = match r.subquery.as_deref().and_then(|n| n.node.as_ref()) {
                    Some(NodeEnum::SelectStmt(q)) => self.select(q)?,
                    _ => return unsupported("subquery in FROM"),
                };
                if let (Some(FromClause::Values { values, .. }), true) = (
                    &query.from_clause,
                    query.where_clause.is_none() && query.order_by.is_none() && query.limit.is_none(),
                ) {
                    return Ok(FromClause::Values {
                        values: values.clone(),
                        alias: Self::alias(&r.alias)?,
                    });
                }
                FromClause::Subquery {
                    query: Box::new(query),
                    alias: Self::alias(&r.alias)?.unwrap_or(TableAlias {
                        name: "subquery".to_string(),
                        columns: None,
                    }),
                    lateral: r.lateral,
                }
            }
            NodeEnum::RangeFunction(r) => {
                if r.is_rowsfrom || r.functions.len() != 1 || !r.coldeflist.is_empty() {
                    return unsupported("ROWS FROM / column definition list");
                }
                let call = match kind(&r.functions[0])? {
                    NodeEnum::List(l) => l.items.first(),
                    _ => None,
                }
                .ok_or_else(|| Unsupported("function in FROM".into()))?;
                let function = match self.expr(call)? {
                    Expression::Function(f) => *f,
                    _ => return unsupported("non-function in FROM"),
                };
                FromClause::TableFunction {
                    function,
                    alias: Self::alias(&r.alias)?,
                    // a function in FROM may read the items to its left
                    lateral: true,
                    with_ordinality: r.ordinality,
                }
            }
            other => return unsupported(format!("FROM item {}", node_name(other))),
        })
    }

    fn join(&self, j: &pb::JoinExpr) -> L<FromClause> {
        use pb::JoinType as T;
        let (Some(l), Some(r)) = (j.larg.as_deref(), j.rarg.as_deref()) else {
            return unsupported("join operands");
        };
        let mut join_type = match T::try_from(j.jointype) {
            Ok(T::JoinInner) => JoinType::Inner,
            Ok(T::JoinLeft) => JoinType::LeftOuter,
            Ok(T::JoinRight) => JoinType::RightOuter,
            Ok(T::JoinFull) => JoinType::FullOuter,
            _ => return unsupported("join type"),
        };
        let condition = if let Some(q) = &j.quals {
            JoinCondition::On(self.expr(q)?)
        } else if !j.using_clause.is_empty() {
            JoinCondition::Using(names(&j.using_clause)?)
        } else {
            if !j.is_natural && join_type == JoinType::Inner {
                join_type = JoinType::Cross;
            }
            JoinCondition::Natural
        };
        Ok(FromClause::Join {
            left: Box::new(self.from_item(l)?),
            join_type,
            right: Box::new(self.from_item(r)?),
            condition,
        })
    }

    fn limit(&self, s: &pb::SelectStmt) -> L<(Option<LimitClause>, Option<u64>)> {
        let is_all = |n: &pb::Node| matches!(n.node.as_ref(), Some(NodeEnum::AConst(c)) if c.isnull);
        let limit = match s.limit_count.as_deref() {
            Some(n) if !is_all(n) => Some(LimitClause {
                count: Some(self.expr(n)?),
                with_ties: s.limit_option == pb::LimitOption::WithTies as i32,
            }),
            _ => None,
        };
        let offset = match s.limit_offset.as_deref() {
            None => None,
            Some(n) => match self.expr(n)? {
                Expression::Literal(SqlValue::Integer(v)) if v >= 0 => Some(v as u64),
                Expression::Literal(SqlValue::BigInt(v)) if v >= 0 => Some(v as u64),
                _ => return unsupported("OFFSET that is not a constant"),
            },
        };
        Ok((limit, offset))
    }

    fn locking(&self, clauses: &[pb::Node]) -> L<Option<ForClause>> {
        use pb::{LockClauseStrength as S, LockWaitPolicy as W};
        let Some(first) = clauses.first() else {
            return Ok(None);
        };
        let NodeEnum::LockingClause(c) = kind(first)? else {
            return unsupported("locking clause");
        };
        let nowait = c.wait_policy == W::LockWaitError as i32;
        let skip_locked = c.wait_policy == W::LockWaitSkip as i32;
        Ok(Some(match S::try_from(c.strength) {
            Ok(S::LcsForupdate | S::LcsFornokeyupdate) => ForClause::Update { nowait, skip_locked },
            Ok(S::LcsForshare | S::LcsForkeyshare) => ForClause::Share { nowait, skip_locked },
            _ => return unsupported("locking strength"),
        }))
    }
}

// ---- DML -------------------------------------------------------------------------------------

fn returning(r: &Option<pb::ReturningClause>) -> L<Option<Vec<SelectItem>>> {
    match r {
        Some(r) if !r.exprs.is_empty() => {
            let cx = Cx::default();
            Ok(Some(
                r.exprs.iter().map(|e| cx.target(e)).collect::<L<_>>()?,
            ))
        }
        _ => Ok(None),
    }
}

fn insert(s: &pb::InsertStmt) -> L<InsertStatement> {
    if s.with_clause.is_some() {
        return unsupported("WITH on INSERT");
    }
    let cx = Cx::default();
    let columns = if s.cols.is_empty() {
        None
    } else {
        Some(
            s.cols
                .iter()
                .map(|c| match kind(c)? {
                    NodeEnum::ResTarget(t) => Ok(t.name.clone()),
                    _ => unsupported("INSERT column"),
                })
                .collect::<L<_>>()?,
        )
    };
    let source = match s.select_stmt.as_deref().and_then(|n| n.node.as_ref()) {
        None => InsertSource::DefaultValues,
        Some(NodeEnum::SelectStmt(q)) if !q.values_lists.is_empty() => InsertSource::Values(
            q.values_lists
                .iter()
                .map(|row| match kind(row)? {
                    NodeEnum::List(l) => l.items.iter().map(|e| cx.expr(e)).collect(),
                    _ => unsupported("VALUES row"),
                })
                .collect::<L<_>>()?,
        ),
        Some(NodeEnum::SelectStmt(q)) => InsertSource::Query(Box::new(cx.select(q)?)),
        Some(_) => return unsupported("INSERT source"),
    };
    let on_conflict = match &s.on_conflict_clause {
        None => None,
        Some(oc) => Some(on_conflict(oc)?),
    };
    Ok(InsertStatement {
        table: opt_table(&s.relation)?,
        columns,
        source,
        on_conflict,
        returning: returning(&s.returning_clause)?,
    })
}

fn assignments(list: &[pb::Node]) -> L<Vec<Assignment>> {
    let cx = Cx::default();
    list.iter()
        .map(|t| match kind(t)? {
            NodeEnum::ResTarget(t) => {
                let val = t
                    .val
                    .as_deref()
                    .ok_or_else(|| Unsupported("empty assignment".into()))?;
                if matches!(val.node.as_ref(), Some(NodeEnum::MultiAssignRef(_))) {
                    return unsupported("multi-column assignment");
                }
                Ok(Assignment {
                    target: AssignmentTarget::Column(t.name.clone()),
                    value: cx.expr(val)?,
                })
            }
            _ => unsupported("assignment"),
        })
        .collect()
}

fn on_conflict(oc: &pb::OnConflictClause) -> L<OnConflictClause> {
    use pb::OnConflictAction as A;
    let cx = Cx::default();
    let target = match &oc.infer {
        None => None,
        Some(i) if !i.conname.is_empty() => Some(ConflictTarget::Constraint(i.conname.clone())),
        Some(i) => {
            if i.where_clause.is_some() {
                return unsupported("partial conflict target");
            }
            Some(ConflictTarget::Columns(
                i.index_elems
                    .iter()
                    .map(|e| match kind(e)? {
                        NodeEnum::IndexElem(e) if e.expr.is_none() => Ok(e.name.clone()),
                        _ => unsupported("expression conflict target"),
                    })
                    .collect::<L<_>>()?,
            ))
        }
    };
    let action = match A::try_from(oc.action) {
        Ok(A::OnconflictNothing) => ConflictAction::DoNothing,
        Ok(A::OnconflictUpdate) => ConflictAction::DoUpdate {
            set: assignments(&oc.target_list)?,
            where_clause: match &oc.where_clause {
                Some(w) => Some(cx.expr(w)?),
                None => None,
            },
        },
        _ => return unsupported("ON CONFLICT action"),
    };
    Ok(OnConflictClause { target, action })
}

fn update(s: &pb::UpdateStmt) -> L<UpdateStatement> {
    if s.with_clause.is_some() {
        return unsupported("WITH on UPDATE");
    }
    let cx = Cx::default();
    let rel = s.relation.as_ref();
    Ok(UpdateStatement {
        table: opt_table(&s.relation)?,
        alias: rel.and_then(|r| r.alias.as_ref()).map(|a| a.aliasname.clone()),
        set: assignments(&s.target_list)?,
        from: if s.from_clause.is_empty() {
            None
        } else {
            Some(
                s.from_clause
                    .iter()
                    .map(|f| cx.from_item(f))
                    .collect::<L<_>>()?,
            )
        },
        where_clause: match &s.where_clause {
            Some(w) => Some(cx.expr(w)?),
            None => None,
        },
        returning: returning(&s.returning_clause)?,
    })
}

fn delete(s: &pb::DeleteStmt) -> L<DeleteStatement> {
    if s.with_clause.is_some() {
        return unsupported("WITH on DELETE");
    }
    let cx = Cx::default();
    let rel = s.relation.as_ref();
    Ok(DeleteStatement {
        table: opt_table(&s.relation)?,
        alias: rel.and_then(|r| r.alias.as_ref()).map(|a| a.aliasname.clone()),
        using: if s.using_clause.is_empty() {
            None
        } else {
            Some(
                s.using_clause
                    .iter()
                    .map(|f| cx.from_item(f))
                    .collect::<L<_>>()?,
            )
        },
        where_clause: match &s.where_clause {
            Some(w) => Some(cx.expr(w)?),
            None => None,
        },
        returning: returning(&s.returning_clause)?,
    })
}

// ---- transactions ----------------------------------------------------------------------------

fn transaction(t: &pb::TransactionStmt) -> L<Statement> {
    use pb::TransactionStmtKind as K;
    let option = |name: &str| -> Option<&pb::DefElem> {
        t.options.iter().find_map(|o| match o.node.as_ref()? {
            NodeEnum::DefElem(d) if d.defname == name => Some(&**d),
            _ => None,
        })
    };
    let text_arg = |d: &pb::DefElem| -> Option<String> {
        match d.arg.as_deref()?.node.as_ref()? {
            NodeEnum::AConst(c) => match c.val.as_ref()? {
                Val::Sval(s) => Some(s.sval.clone()),
                _ => None,
            },
            _ => None,
        }
    };
    let int_arg = |d: &pb::DefElem| -> Option<i32> {
        match d.arg.as_deref()?.node.as_ref()? {
            NodeEnum::AConst(c) => match c.val.as_ref()? {
                Val::Ival(i) => Some(i.ival),
                _ => None,
            },
            _ => None,
        }
    };
    Ok(match K::try_from(t.kind) {
        Ok(K::TransStmtBegin | K::TransStmtStart) => {
            let isolation_level = match option("transaction_isolation").and_then(text_arg) {
                None => None,
                Some(level) => Some(match level.as_str() {
                    "read uncommitted" => IsolationLevel::ReadUncommitted,
                    "read committed" => IsolationLevel::ReadCommitted,
                    "repeatable read" => IsolationLevel::RepeatableRead,
                    "serializable" => IsolationLevel::Serializable,
                    _ => return unsupported("isolation level"),
                }),
            };
            let access_mode = option("transaction_read_only")
                .and_then(int_arg)
                .map(|ro| if ro != 0 { AccessMode::ReadOnly } else { AccessMode::ReadWrite });
            Statement::Begin(BeginStatement {
                isolation_level,
                access_mode,
            })
        }
        Ok(K::TransStmtCommit) => Statement::Commit(CommitStatement { chain: t.chain }),
        Ok(K::TransStmtRollback) => Statement::Rollback(RollbackStatement {
            to_savepoint: None,
            chain: t.chain,
        }),
        Ok(K::TransStmtRollbackTo) => Statement::Rollback(RollbackStatement {
            to_savepoint: Some(savepoint_name(t)?),
            chain: false,
        }),
        Ok(K::TransStmtSavepoint) => Statement::Savepoint(SavepointStatement {
            name: savepoint_name(t)?,
        }),
        Ok(K::TransStmtRelease) => Statement::ReleaseSavepoint(ReleaseSavepointStatement {
            name: savepoint_name(t)?,
        }),
        _ => return unsupported("two-phase commit statement"),
    })
}

fn savepoint_name(t: &pb::TransactionStmt) -> L<String> {
    if !t.savepoint_name.is_empty() {
        return Ok(t.savepoint_name.clone());
    }
    t.options
        .iter()
        .find_map(|o| match o.node.as_ref()? {
            NodeEnum::DefElem(d) if d.defname == "savepoint_name" => {
                match d.arg.as_deref()?.node.as_ref()? {
                    NodeEnum::String(s) => Some(s.sval.clone()),
                    NodeEnum::AConst(c) => match c.val.as_ref()? {
                        Val::Sval(s) => Some(s.sval.clone()),
                        _ => None,
                    },
                    _ => None,
                }
            }
            _ => None,
        })
        .ok_or_else(|| Unsupported("savepoint without a name".into()))
}

// ---- DDL -------------------------------------------------------------------------------------

fn referential_action(c: &str) -> Option<ReferentialAction> {
    match c {
        "c" => Some(ReferentialAction::Cascade),
        "n" => Some(ReferentialAction::SetNull),
        "d" => Some(ReferentialAction::SetDefault),
        "r" => Some(ReferentialAction::Restrict),
        // 'a' (NO ACTION) is also what an absent clause means
        _ => None,
    }
}

fn column_constraint(c: &pb::Constraint) -> L<Option<ColumnConstraint>> {
    use pb::ConstrType as T;
    let cx = Cx::default();
    Ok(Some(match T::try_from(c.contype) {
        Ok(T::ConstrNull) => ColumnConstraint::Null,
        Ok(T::ConstrNotnull) => ColumnConstraint::NotNull,
        Ok(T::ConstrDefault) => ColumnConstraint::Default(cx.expr(
            c.raw_expr
                .as_deref()
                .ok_or_else(|| Unsupported("DEFAULT without an expression".into()))?,
        )?),
        Ok(T::ConstrPrimary) => ColumnConstraint::PrimaryKey,
        Ok(T::ConstrUnique) => ColumnConstraint::Unique,
        Ok(T::ConstrCheck) => ColumnConstraint::Check(cx.expr(
            c.raw_expr
                .as_deref()
                .ok_or_else(|| Unsupported("CHECK without an expression".into()))?,
        )?),
        Ok(T::ConstrIdentity) => ColumnConstraint::Identity {
            always: c.generated_when == "a",
        },
        Ok(T::ConstrGenerated) => ColumnConstraint::Generated {
            expression: cx.expr(
                c.raw_expr
                    .as_deref()
                    .ok_or_else(|| Unsupported("generated column without an expression".into()))?,
            )?,
            storage: if c.generated_kind == "v" {
                GeneratedColumnStorage::Virtual
            } else {
                GeneratedColumnStorage::Stored
            },
        },
        Ok(T::ConstrForeign) => ColumnConstraint::References {
            table: opt_table(&c.pktable)?,
            columns: if c.pk_attrs.is_empty() {
                None
            } else {
                Some(names(&c.pk_attrs)?)
            },
            on_delete: referential_action(&c.fk_del_action),
            on_update: referential_action(&c.fk_upd_action),
        },
        Ok(
            T::ConstrAttrDeferrable
            | T::ConstrAttrNotDeferrable
            | T::ConstrAttrDeferred
            | T::ConstrAttrImmediate
            | T::ConstrAttrEnforced
            | T::ConstrAttrNotEnforced,
        ) => return Ok(None),
        _ => return unsupported("column constraint"),
    }))
}

fn table_constraint(c: &pb::Constraint) -> L<TableConstraint> {
    use pb::ConstrType as T;
    let cx = Cx::default();
    let name = (!c.conname.is_empty()).then(|| c.conname.clone());
    let overlaps = || -> L<Option<String>> {
        if c.without_overlaps {
            names(&c.keys).map(|k| k.last().cloned())
        } else {
            Ok(None)
        }
    };
    Ok(match T::try_from(c.contype) {
        Ok(T::ConstrPrimary) => TableConstraint::PrimaryKey {
            name,
            columns: names(&c.keys)?,
            without_overlaps: overlaps()?,
        },
        Ok(T::ConstrUnique) => TableConstraint::Unique {
            name,
            columns: names(&c.keys)?,
            without_overlaps: overlaps()?,
        },
        Ok(T::ConstrCheck) => TableConstraint::Check {
            name,
            expression: cx.expr(
                c.raw_expr
                    .as_deref()
                    .ok_or_else(|| Unsupported("CHECK without an expression".into()))?,
            )?,
        },
        Ok(T::ConstrForeign) => TableConstraint::ForeignKey {
            name,
            columns: names(&c.fk_attrs)?,
            references_table: opt_table(&c.pktable)?,
            references_columns: names(&c.pk_attrs)?,
            on_delete: referential_action(&c.fk_del_action),
            on_update: referential_action(&c.fk_upd_action),
            period_column: None,
            references_period: None,
        },
        _ => return unsupported("table constraint"),
    })
}

fn column_def(d: &pb::ColumnDef) -> L<ColumnDefinition> {
    let tn = d
        .type_name
        .as_ref()
        .ok_or_else(|| Unsupported("column without a type".into()))?;
    let mut constraints = Vec::new();
    // serial pseudo-types become an integer column owned by a sequence
    let serial = tn.names.last().and_then(string_of).map(str::to_ascii_lowercase);
    let data_type = match serial.as_deref() {
        Some("serial" | "serial4") => {
            constraints.push(ColumnConstraint::Serial);
            constraints.push(ColumnConstraint::NotNull);
            SqlType::Integer
        }
        Some("bigserial" | "serial8") => {
            constraints.push(ColumnConstraint::Serial);
            constraints.push(ColumnConstraint::NotNull);
            SqlType::BigInt
        }
        Some("smallserial" | "serial2") => {
            constraints.push(ColumnConstraint::Serial);
            constraints.push(ColumnConstraint::NotNull);
            SqlType::SmallInt
        }
        _ => sql_type(tn)?,
    };
    if d.is_not_null {
        constraints.push(ColumnConstraint::NotNull);
    }
    for c in &d.constraints {
        match kind(c)? {
            NodeEnum::Constraint(c) => {
                if let Some(lowered) = column_constraint(c)? {
                    constraints.push(lowered);
                }
            }
            _ => return unsupported("column constraint"),
        }
    }
    if d.coll_clause.is_some() {
        return unsupported("COLLATE on a column");
    }
    Ok(ColumnDefinition {
        name: d.colname.clone(),
        data_type,
        constraints,
    })
}

fn create_table(s: &pb::CreateStmt) -> L<CreateTableStatement> {
    if !s.inh_relations.is_empty()
        || s.partspec.is_some()
        || s.partbound.is_some()
        || s.of_typename.is_some()
        || !s.options.is_empty()
    {
        return unsupported("inheritance, partitioning, typed tables or storage options");
    }
    let mut columns = Vec::new();
    let mut constraints = Vec::new();
    for elt in &s.table_elts {
        match kind(elt)? {
            NodeEnum::ColumnDef(d) => columns.push(column_def(d)?),
            NodeEnum::Constraint(c) => constraints.push(table_constraint(c)?),
            _ => return unsupported("table element"),
        }
    }
    Ok(CreateTableStatement {
        if_not_exists: s.if_not_exists,
        name: opt_table(&s.relation)?,
        columns,
        constraints,
        options: Vec::new(),
        as_query: None,
    })
}

fn create_table_as(s: &pb::CreateTableAsStmt) -> L<CreateTableStatement> {
    if s.objtype != pb::ObjectType::ObjectTable as i32 || s.is_select_into {
        return unsupported("materialized view or SELECT INTO");
    }
    let into = s
        .into
        .as_deref()
        .ok_or_else(|| Unsupported("CREATE TABLE AS without a target".into()))?;
    if !into.col_names.is_empty() || into.skip_data {
        return unsupported("CREATE TABLE AS column list or WITH NO DATA");
    }
    let query = match s.query.as_deref().and_then(|n| n.node.as_ref()) {
        Some(NodeEnum::SelectStmt(q)) => Cx::default().select(q)?,
        _ => return unsupported("CREATE TABLE AS source"),
    };
    Ok(CreateTableStatement {
        if_not_exists: s.if_not_exists,
        name: opt_table(&into.rel)?,
        columns: Vec::new(),
        constraints: Vec::new(),
        options: Vec::new(),
        as_query: Some(Box::new(query)),
    })
}

fn create_index(s: &pb::IndexStmt) -> L<CreateIndexStatement> {
    let cx = Cx::default();
    if !s.index_including_params.is_empty() || !s.options.is_empty() {
        return unsupported("INCLUDE or index storage options");
    }
    let columns = s
        .index_params
        .iter()
        .map(|p| match kind(p)? {
            NodeEnum::IndexElem(e) if e.expr.is_none() => {
                use pb::{SortByDir as D, SortByNulls as N};
                Ok(IndexColumn {
                    name: e.name.clone(),
                    direction: match D::try_from(e.ordering) {
                        Ok(D::SortbyAsc) => Some(SortDirection::Ascending),
                        Ok(D::SortbyDesc) => Some(SortDirection::Descending),
                        _ => None,
                    },
                    nulls: match N::try_from(e.nulls_ordering) {
                        Ok(N::SortbyNullsFirst) => Some(NullsOrder::First),
                        Ok(N::SortbyNullsLast) => Some(NullsOrder::Last),
                        _ => None,
                    },
                })
            }
            _ => unsupported("expression index"),
        })
        .collect::<L<_>>()?;
    let index_type = match s.access_method.as_str() {
        "" | "btree" => IndexType::BTree,
        "hash" => IndexType::Hash,
        "gist" => IndexType::Gist,
        "gin" => IndexType::Gin,
        "ivfflat" => IndexType::IvfFlat { lists: None },
        "hnsw" => IndexType::Hnsw {
            m: None,
            ef_construction: None,
        },
        other => return unsupported(format!("index method {other}")),
    };
    Ok(CreateIndexStatement {
        if_not_exists: s.if_not_exists,
        unique: s.unique,
        name: (!s.idxname.is_empty()).then(|| s.idxname.clone()),
        table: opt_table(&s.relation)?,
        columns,
        index_type,
        where_clause: match &s.where_clause {
            Some(w) => Some(cx.expr(w)?),
            None => None,
        },
        options: Vec::new(),
    })
}

fn create_view(s: &pb::ViewStmt) -> L<CreateViewStatement> {
    if !s.options.is_empty() || s.with_check_option != 0 {
        return unsupported("view options");
    }
    let query = match s.query.as_deref().and_then(|n| n.node.as_ref()) {
        Some(NodeEnum::SelectStmt(q)) => Cx::default().select(q)?,
        _ => return unsupported("view query"),
    };
    Ok(CreateViewStatement {
        if_not_exists: false,
        name: opt_table(&s.view)?,
        columns: if s.aliases.is_empty() {
            None
        } else {
            Some(names(&s.aliases)?)
        },
        query: Box::new(query),
        materialized: false,
        replace: s.replace,
    })
}

fn drop_object(s: &pb::DropStmt) -> L<Statement> {
    use pb::ObjectType as O;
    let cascade = s.behavior == pb::DropBehavior::DropCascade as i32;
    // each object is a list of name parts: [schema, name] or [name]
    let object_names = || -> L<Vec<Vec<String>>> {
        s.objects
            .iter()
            .map(|o| match kind(o)? {
                NodeEnum::List(l) => names(&l.items),
                _ => unsupported("object name"),
            })
            .collect()
    };
    let as_table = |parts: Vec<String>| match parts.as_slice() {
        [name] => TableName::new(name.clone()),
        [schema, name] => TableName::with_schema(schema.clone(), name.clone()),
        _ => TableName::new(parts.join(".")),
    };
    Ok(match O::try_from(s.remove_type) {
        Ok(O::ObjectTable) => Statement::DropTable(DropTableStatement {
            if_exists: s.missing_ok,
            names: object_names()?.into_iter().map(as_table).collect(),
            cascade,
        }),
        Ok(O::ObjectView) => Statement::DropView(DropViewStatement {
            if_exists: s.missing_ok,
            names: object_names()?.into_iter().map(as_table).collect(),
            cascade,
            materialized: false,
        }),
        Ok(O::ObjectIndex) => Statement::DropIndex(DropIndexStatement {
            if_exists: s.missing_ok,
            names: object_names()?
                .into_iter()
                .map(|p| p.last().cloned().unwrap_or_default())
                .collect(),
            cascade,
        }),
        _ => return unsupported("DROP of this object kind"),
    })
}

fn truncate(s: &pb::TruncateStmt) -> L<TruncateStatement> {
    Ok(TruncateStatement {
        tables: s
            .relations
            .iter()
            .map(|r| match kind(r)? {
                NodeEnum::RangeVar(rv) => Ok(table_name(rv)),
                _ => unsupported("TRUNCATE target"),
            })
            .collect::<L<_>>()?,
        identity: s.restart_seqs.then_some(TruncateIdentity::Restart),
        cascade: (s.behavior == pb::DropBehavior::DropCascade as i32).then_some(true),
        only: false,
    })
}

fn alter_table(s: &pb::AlterTableStmt) -> L<AlterTableStatement> {
    use pb::AlterTableType as T;
    if s.objtype != pb::ObjectType::ObjectTable as i32 {
        return unsupported("ALTER of this object kind");
    }
    let actions = s
        .cmds
        .iter()
        .map(|c| {
            let NodeEnum::AlterTableCmd(cmd) = kind(c)? else {
                return unsupported("ALTER TABLE command");
            };
            let cascade = cmd.behavior == pb::DropBehavior::DropCascade as i32;
            Ok(match T::try_from(cmd.subtype) {
                Ok(T::AtAddColumn) => match cmd.def.as_deref().and_then(|d| d.node.as_ref()) {
                    Some(NodeEnum::ColumnDef(d)) => AlterTableAction::AddColumn(column_def(d)?),
                    _ => return unsupported("ADD COLUMN"),
                },
                Ok(T::AtDropColumn) => AlterTableAction::DropColumn {
                    name: cmd.name.clone(),
                    if_exists: cmd.missing_ok,
                    cascade,
                },
                Ok(T::AtAddConstraint) => match cmd.def.as_deref().and_then(|d| d.node.as_ref()) {
                    Some(NodeEnum::Constraint(c)) => {
                        AlterTableAction::AddConstraint(table_constraint(c)?)
                    }
                    _ => return unsupported("ADD CONSTRAINT"),
                },
                Ok(T::AtDropConstraint) => AlterTableAction::DropConstraint {
                    name: cmd.name.clone(),
                    cascade,
                },
                Ok(T::AtColumnDefault) => AlterTableAction::AlterColumn {
                    name: cmd.name.clone(),
                    action: match &cmd.def {
                        Some(d) => AlterColumnAction::SetDefault(Cx::default().expr(d)?),
                        None => AlterColumnAction::DropDefault,
                    },
                },
                Ok(T::AtSetNotNull) => AlterTableAction::AlterColumn {
                    name: cmd.name.clone(),
                    action: AlterColumnAction::SetNotNull,
                },
                Ok(T::AtDropNotNull) => AlterTableAction::AlterColumn {
                    name: cmd.name.clone(),
                    action: AlterColumnAction::DropNotNull,
                },
                Ok(T::AtAlterColumnType) => match cmd.def.as_deref().and_then(|d| d.node.as_ref()) {
                    Some(NodeEnum::ColumnDef(d)) => AlterTableAction::AlterColumn {
                        name: cmd.name.clone(),
                        action: AlterColumnAction::SetType(
                            sql_type(d.type_name.as_ref().ok_or_else(|| {
                                Unsupported("ALTER COLUMN TYPE without a type".into())
                            })?)?,
                        ),
                    },
                    _ => return unsupported("ALTER COLUMN TYPE"),
                },
                _ => return unsupported("this ALTER TABLE command"),
            })
        })
        .collect::<L<_>>()?;
    Ok(AlterTableStatement {
        name: opt_table(&s.relation)?,
        actions,
    })
}

fn rename(s: &pb::RenameStmt) -> L<Statement> {
    use pb::ObjectType as O;
    let table = opt_table(&s.relation)?;
    let action = match O::try_from(s.rename_type) {
        Ok(O::ObjectColumn) => AlterTableAction::RenameColumn {
            old_name: s.subname.clone(),
            new_name: s.newname.clone(),
        },
        Ok(O::ObjectTable) => AlterTableAction::RenameTable(s.newname.clone()),
        _ => return unsupported("RENAME of this object kind"),
    };
    Ok(Statement::AlterTable(AlterTableStatement {
        name: table,
        actions: vec![action],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::postgres_wire::sql::parser::SqlParser;
    use std::path::Path;

    /// Every statement in the conformance corpus (`tests/conformance/slt` and `/sql`).
    fn corpus() -> Vec<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(root.join("slt")).expect("slt dir").flatten() {
            let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
            let mut lines = text.lines().peekable();
            while let Some(line) = lines.next() {
                if line.starts_with("statement") || line.starts_with("query") {
                    let mut sql = Vec::new();
                    while let Some(&l) = lines.peek() {
                        if l.trim().is_empty() || l == "----" {
                            break;
                        }
                        sql.push(lines.next().unwrap());
                    }
                    out.push(sql.join("\n"));
                }
            }
        }
        out.retain(|s| !s.is_empty());
        out.sort();
        out.dedup();
        out
    }

    /// Compare ignoring case: PostgreSQL's grammar folds unquoted names to lower case, Orbit's keeps them.
    fn shape(s: &[Statement]) -> String {
        format!("{s:?}").to_ascii_lowercase()
    }

    #[test]
    #[ignore = "report: cargo test -p orbit-server --features pg-query-parser --lib lowering_report -- --ignored --nocapture"]
    fn lowering_report() {
        let (mut same, mut differ, mut unsupported, mut syntax, mut hand_only) = (0, vec![], vec![], vec![], vec![]);
        let all = corpus();
        for sql in &all {
            let hand = SqlParser::new().parse_multiple_hand_rolled(sql);
            match (lower(sql), hand) {
                (Lowered::Statements(l), Ok(h)) => {
                    if shape(&l) == shape(&h) {
                        same += 1;
                    } else {
                        differ.push((sql.clone(), shape(&l), shape(&h)));
                    }
                }
                (Lowered::Statements(_), Err(_)) => hand_only.push(sql.clone()),
                (Lowered::Unsupported(why), _) => unsupported.push((why, sql.clone())),
                (Lowered::SyntaxError(m), _) => syntax.push((m, sql.clone())),
            }
        }
        println!("\nlowering: {} statements; identical {same}; differ {}; hand-rolled rejects {}; unsupported {}; PostgreSQL syntax errors {}", all.len(), differ.len(), hand_only.len(), unsupported.len(), syntax.len());
        for (sql, l, h) in &differ {
            let at = l.chars().zip(h.chars()).position(|(a, b)| a != b).unwrap_or(0);
            let ctx = |s: &str| s.chars().skip(at.saturating_sub(60)).take(170).collect::<String>();
            println!("DIFFER {}\n   pg  : …{}\n   hand: …{}", sql.replace('\n', " ").chars().take(110).collect::<String>(), ctx(l), ctx(h));
        }
        for sql in &hand_only {
            println!("ONLY-PG {}", sql.replace('\n', " ").chars().take(120).collect::<String>());
        }
        let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
        for (why, _) in &unsupported {
            *reasons.entry(why.clone()).or_default() += 1;
        }
        for (why, n) in reasons {
            println!("UNSUPPORTED {n:>3} {why}");
        }
    }
}
