//! AST for Stored Procedures (PL/pgSQL subset)

use serde::{Deserialize, Serialize};

/// Stored Procedure Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureDef {
    pub name: String,
    pub args: Vec<ArgDef>,
    pub return_type: String,
    pub body: Block,
    pub language: String, // e.g., "plpgsql"
}

/// Argument Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgDef {
    pub name: String,
    pub type_name: String,
}

/// Block of statements (BEGIN ... END)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub statements: Vec<Statement>,
}

/// Procedural Statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Statement {
    /// Variable declaration (DECLARE name type [= expr];)
    Declaration {
        name: String,
        type_name: String,
        default_value: Option<Expression>,
    },
    /// Assignment (name := expr;)
    Assignment { name: String, value: Expression },
    /// If-Then-Else
    If {
        condition: Expression,
        then_block: Block,
        else_block: Option<Block>,
    },
    /// Loop (LOOP ... END LOOP;)
    Loop { body: Block },
    /// While Loop (WHILE cond LOOP ... END LOOP;)
    While { condition: Expression, body: Block },
    /// Return statement
    Return { value: Option<Expression> },
    /// SQL Statement (executed via query engine)
    Sql {
        query: String,
        // Bind parameters from variables?
        params: Vec<String>,
    },
    /// Raise notice/exception
    Raise {
        level: String, // NOTICE, EXCEPTION
        message: String,
    },
}

/// Expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expression {
    Literal(Literal),
    Variable(String),
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    UnaryOp {
        op: UnaryOperator,
        operand: Box<Expression>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expression>,
    },
}

/// Literal Value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
}

/// Binary Operator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    And,
    Or,
}

/// Unary Operator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UnaryOperator {
    Not,
    Negate,
}
