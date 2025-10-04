//! Expression Parser Implementation
//! 
//! This module handles parsing of SQL expressions with proper precedence and associativity

use crate::postgres_wire::sql::ast::*;
use crate::postgres_wire::sql::lexer::Token;
use crate::error::ProtocolResult;

/// Expression parser with operator precedence handling
pub struct ExpressionParser;

impl ExpressionParser {
    pub fn new() -> Self {
        Self
    }
    
    /// Parse a complete expression with proper precedence
    pub fn parse_expression(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<Expression> {
        self.parse_or_expression(tokens, pos)
    }
    
    /// Parse OR expressions (lowest precedence)
    fn parse_or_expression(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<Expression> {
        let mut left = self.parse_and_expression(tokens, pos)?;
        
        while *pos < tokens.len() {
            if matches!(tokens[*pos], Token::Or) {
                *pos += 1;
                let right = self.parse_and_expression(tokens, pos)?;
                left = Expression::BinaryOp {
                    left: Box::new(left),
                    operator: BinaryOperator::Or,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        
        Ok(left)
    }
    
    /// Parse AND expressions
    fn parse_and_expression(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<Expression> {
        let mut left = self.parse_equality_expression(tokens, pos)?;
        
        while *pos < tokens.len() {
            if matches!(tokens[*pos], Token::And) {
                *pos += 1;
                let right = self.parse_equality_expression(tokens, pos)?;
                left = Expression::BinaryOp {
                    left: Box::new(left),
                    operator: BinaryOperator::And,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        
        Ok(left)
    }
    
    /// Parse equality expressions (=, !=, <>, IS, IS NOT)
    fn parse_equality_expression(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<Expression> {
        let mut left = self.parse_comparison_expression(tokens, pos)?;
        
        while *pos < tokens.len() {
            let operator = match &tokens[*pos] {
                Token::Equal => BinaryOperator::Equal,
                Token::NotEqual => BinaryOperator::NotEqual,
                Token::Is => {
                    *pos += 1;
                    if *pos < tokens.len() && matches!(tokens[*pos], Token::Not) {
                        *pos += 1;
                        BinaryOperator::IsNot
                    } else {
                        BinaryOperator::Is
                    }
                }
                _ => break,
            };
            
            if !matches!(operator, BinaryOperator::Is | BinaryOperator::IsNot) {
                *pos += 1;
            }
            
            let right = self.parse_comparison_expression(tokens, pos)?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }
        
        Ok(left)
    }
    
    /// Parse comparison expressions (<, <=, >, >=, LIKE, IN, BETWEEN)
    fn parse_comparison_expression(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<Expression> {
        let mut left = self.parse_additive_expression(tokens, pos)?;
        
        while *pos < tokens.len() {
            let operator = match &tokens[*pos] {
                Token::LessThan => BinaryOperator::LessThan,
                Token::LessThanOrEqual => BinaryOperator::LessThanOrEqual,
                Token::GreaterThan => BinaryOperator::GreaterThan,
                Token::GreaterThanOrEqual => BinaryOperator::GreaterThanOrEqual,
                Token::Like => BinaryOperator::Like,
                Token::ILike => BinaryOperator::ILike,
                Token::In => BinaryOperator::In,
                Token::VectorDistance => BinaryOperator::VectorDistance,
                Token::VectorInnerProduct => BinaryOperator::VectorInnerProduct,
                Token::VectorCosineDistance => BinaryOperator::VectorCosineDistance,
                _ => break,
            };
            
            *pos += 1;
            let right = self.parse_additive_expression(tokens, pos)?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }
        
        Ok(left)
    }
    
    /// Parse additive expressions (+, -, ||)
    fn parse_additive_expression(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<Expression> {
        let mut left = self.parse_multiplicative_expression(tokens, pos)?;
        
        while *pos < tokens.len() {
            let operator = match &tokens[*pos] {
                Token::Plus => BinaryOperator::Plus,
                Token::Minus => BinaryOperator::Minus,
                Token::Concat => BinaryOperator::Concat,
                _ => break,
            };
            
            *pos += 1;
            let right = self.parse_multiplicative_expression(tokens, pos)?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }
        
        Ok(left)
    }
    
    /// Parse multiplicative expressions (*, /, %)
    fn parse_multiplicative_expression(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<Expression> {
        let mut left = self.parse_unary_expression(tokens, pos)?;
        
        while *pos < tokens.len() {
            let operator = match &tokens[*pos] {
                Token::Multiply => BinaryOperator::Multiply,
                Token::Divide => BinaryOperator::Divide,
                Token::Modulo => BinaryOperator::Modulo,
                _ => break,
            };
            
            *pos += 1;
            let right = self.parse_unary_expression(tokens, pos)?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }
        
        Ok(left)
    }
    
    /// Parse unary expressions (NOT, -, +)
    fn parse_unary_expression(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<Expression> {
        if *pos >= tokens.len() {
            return Err(crate::error::ProtocolError::ParseError("Unexpected end of tokens".to_string()).into());
        }
        
        match &tokens[*pos] {
            Token::Not => {
                *pos += 1;
                let expr = self.parse_unary_expression(tokens, pos)?;
                Ok(Expression::UnaryOp {
                    operator: UnaryOperator::Not,
                    operand: Box::new(expr),
                })
            }
            Token::Minus => {
                *pos += 1;
                let expr = self.parse_unary_expression(tokens, pos)?;
                Ok(Expression::UnaryOp {
                    operator: UnaryOperator::Minus,
                    operand: Box::new(expr),
                })
            }
            Token::Plus => {
                *pos += 1;
                let expr = self.parse_unary_expression(tokens, pos)?;
                Ok(Expression::UnaryOp {
                    operator: UnaryOperator::Plus,
                    operand: Box::new(expr),
                })
            }
            _ => self.parse_primary_expression(tokens, pos),
        }
    }
    
    /// Parse primary expressions (literals, identifiers, function calls, parenthesized expressions)
    fn parse_primary_expression(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<Expression> {
        if *pos >= tokens.len() {
            return Err(crate::error::ProtocolError::ParseError("Unexpected end of tokens".to_string()).into());
        }
        
        match &tokens[*pos] {
            Token::StringLiteral(s) => {
                *pos += 1;
                Ok(Expression::Literal(Literal::String(s.clone())))
            }
            Token::NumericLiteral(n) => {
                *pos += 1;
                Ok(Expression::Literal(Literal::Number(n.clone())))
            }
            Token::BooleanLiteral(b) => {
                *pos += 1;
                Ok(Expression::Literal(Literal::Boolean(*b)))
            }
            Token::Null => {
                *pos += 1;
                Ok(Expression::Literal(Literal::Null))
            }
            Token::Identifier(name) => {
                *pos += 1;
                
                // Check for function call
                if *pos < tokens.len() && matches!(tokens[*pos], Token::LeftParen) {
                    *pos += 1; // consume '('
                    
                    let mut args = Vec::new();
                    
                    // Parse arguments
                    if *pos < tokens.len() && !matches!(tokens[*pos], Token::RightParen) {
                        loop {
                            args.push(self.parse_expression(tokens, pos)?);
                            
                            if *pos < tokens.len() && matches!(tokens[*pos], Token::Comma) {
                                *pos += 1; // consume ','
                            } else {
                                break;
                            }
                        }
                    }
                    
                    if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightParen) {
                        return Err(crate::error::ProtocolError::ParseError("Expected ')' after function arguments".to_string()).into());
                    }
                    *pos += 1; // consume ')'
                    
                    Ok(Expression::FunctionCall {
                        name: name.clone(),
                        args,
                    })
                } else {
                    // Regular column reference
                    Ok(Expression::Identifier {
                        table: None,
                        column: name.clone(),
                    })
                }
            }
            Token::LeftParen => {
                *pos += 1; // consume '('
                let expr = self.parse_expression(tokens, pos)?;
                
                if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RightParen) {
                    return Err(crate::error::ProtocolError::ParseError("Expected ')' after expression".to_string()).into());
                }
                *pos += 1; // consume ')'
                
                Ok(expr)
            }
            _ => Err(crate::error::ProtocolError::ParseError(format!("Unexpected token in expression: {:?}", tokens[*pos])).into()),
        }
    }
}

impl Default for ExpressionParser {
    fn default() -> Self {
        Self::new()
    }
}
