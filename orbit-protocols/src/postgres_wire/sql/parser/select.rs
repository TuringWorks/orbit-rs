//! SELECT Statement Parser Implementation

use crate::postgres_wire::sql::ast::*;
use crate::postgres_wire::sql::lexer::Token;
use crate::postgres_wire::sql::parser::expressions::ExpressionParser;
use crate::error::ProtocolResult;

/// Parser for SELECT statements
pub struct SelectParser {
    expression_parser: ExpressionParser,
}

impl SelectParser {
    pub fn new() -> Self {
        Self {
            expression_parser: ExpressionParser::new(),
        }
    }

    /// Parse a minimal SELECT statement for now
    pub fn parse_select(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<SelectStatement> {
        // Expect SELECT keyword
        if *pos >= tokens.len() || !matches!(tokens[*pos], Token::Select) {
            return Err(crate::error::ProtocolError::ParseError("Expected SELECT".to_string()).into());
        }
        *pos += 1;
        
        // Parse SELECT list - for now just handle simple cases
        let select_list = self.parse_simple_select_list(tokens, pos)?;
        
        // Parse FROM clause if present
        let from_clause = if *pos < tokens.len() && matches!(tokens[*pos], Token::From) {
            *pos += 1; // consume FROM
            Some(self.parse_simple_from_clause(tokens, pos)?)
        } else {
            None
        };
        
        // Parse WHERE clause if present
        let where_clause = if *pos < tokens.len() && matches!(tokens[*pos], Token::Where) {
            *pos += 1; // consume WHERE
            Some(self.expression_parser.parse_expression(tokens, pos)?)
        } else {
            None
        };
        
        // Parse LIMIT clause if present
        let limit = if *pos < tokens.len() && matches!(tokens[*pos], Token::Limit) {
            *pos += 1; // consume LIMIT
            let count = Some(self.expression_parser.parse_expression(tokens, pos)?);
            Some(LimitClause { count, with_ties: false })
        } else {
            None
        };
        
        Ok(SelectStatement {
            with: None,
            select_list,
            distinct: None,
            from_clause,
            where_clause,
            group_by: None,
            having: None,
            order_by: None,
            limit,
            offset: None,
            for_clause: None,
        })
    }

    fn parse_simple_select_list(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<Vec<SelectItem>> {
        let mut select_items = Vec::new();

        loop {
            if *pos >= tokens.len() {
                break;
            }

            // Check for wildcard
            if matches!(tokens[*pos], Token::Multiply) {
                *pos += 1;
                select_items.push(SelectItem::Wildcard);
            } else if let Token::Identifier(name) = &tokens[*pos] {
                // Simple column reference
                let expr = Expression::Column(ColumnRef {
                    table: None,
                    name: name.clone(),
                });
                *pos += 1;
                
                // Check for alias
                let alias = if *pos < tokens.len() && matches!(tokens[*pos], Token::As) {
                    *pos += 1; // consume AS
                    if let Token::Identifier(alias_name) = &tokens[*pos] {
                        *pos += 1;
                        Some(alias_name.clone())
                    } else {
                        None
                    }
                } else {
                    None
                };
                
                select_items.push(SelectItem::Expression { expr, alias });
            } else {
                // Try to parse as expression
                let expr = self.expression_parser.parse_expression(tokens, pos)?;
                select_items.push(SelectItem::Expression { expr, alias: None });
            }

            // Check for comma
            if *pos < tokens.len() && matches!(tokens[*pos], Token::Comma) {
                *pos += 1;
            } else {
                break;
            }
        }

        if select_items.is_empty() {
            return Err(crate::error::ProtocolError::ParseError("Expected SELECT list".to_string()).into());
        }

        Ok(select_items)
    }

    fn parse_simple_from_clause(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<FromClause> {
        if *pos >= tokens.len() {
            return Err(crate::error::ProtocolError::ParseError("Expected table name".to_string()).into());
        }

        if let Token::Identifier(table_name) = &tokens[*pos] {
            let table = TableName {
                schema: None,
                name: table_name.clone(),
            };
            *pos += 1;

            Ok(FromClause::Table { 
                name: table, 
                alias: None 
            })
        } else {
            Err(crate::error::ProtocolError::ParseError("Expected table name".to_string()).into())
        }
    }
}

impl Default for SelectParser {
    fn default() -> Self {
        Self::new()
    }
}