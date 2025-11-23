//! SELECT Statement Parser Implementation

#![allow(clippy::useless_conversion)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::while_let_loop)]
#![allow(clippy::unnecessary_map_or)]

use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::sql::ast::{
    CommonTableExpression, DistinctClause, Expression, FromClause, JoinCondition, JoinType,
    LimitClause, NullsOrder, OrderByItem, SelectItem, SelectStatement, SortDirection, TableAlias,
    TableName, WithClause,
};
use crate::protocols::postgres_wire::sql::lexer::Token;
use crate::protocols::postgres_wire::sql::parser::expressions::ExpressionParser;

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

    /// Parse a comprehensive SELECT statement
    pub fn parse_select(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<SelectStatement> {
        // Parse WITH clause if present
        let with = if self.matches_at(tokens, *pos, &Token::With) {
            Some(self.parse_with_clause(tokens, pos)?)
        } else {
            None
        };

        // Expect SELECT keyword
        if *pos >= tokens.len() || !matches!(tokens[*pos], Token::Select) {
            return Err(
                crate::protocols::error::ProtocolError::ParseError("Expected SELECT".to_string()).into(),
            );
        }
        *pos += 1;

        // Parse DISTINCT/ALL
        let distinct = if self.matches_at(tokens, *pos, &Token::Distinct) {
            *pos += 1;
            if self.matches_at(tokens, *pos, &Token::On) {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;
                let exprs = self.parse_expression_list(tokens, pos)?;
                self.expect_token(tokens, pos, &Token::RightParen)?;
                Some(DistinctClause::DistinctOn(exprs))
            } else {
                Some(DistinctClause::Distinct)
            }
        } else if self.matches_at(tokens, *pos, &Token::All) {
            *pos += 1;
            None // ALL is default
        } else {
            None
        };

        // Parse SELECT list
        let select_list = self.parse_select_list(tokens, pos)?;

        // Parse FROM clause if present
        let from_clause = if self.matches_at(tokens, *pos, &Token::From) {
            *pos += 1;
            Some(self.parse_from_clause(tokens, pos)?)
        } else {
            None
        };

        // Parse WHERE clause if present
        let where_clause = if self.matches_at(tokens, *pos, &Token::Where) {
            *pos += 1;
            Some(self.expression_parser.parse_expression(tokens, pos)?)
        } else {
            None
        };

        // Parse GROUP BY clause if present
        let group_by = if self.matches_at(tokens, *pos, &Token::Group) {
            *pos += 1;
            self.expect_token(tokens, pos, &Token::By)?;
            Some(self.parse_expression_list(tokens, pos)?)
        } else {
            None
        };

        // Parse HAVING clause if present
        let having = if self.matches_at(tokens, *pos, &Token::Having) {
            *pos += 1;
            Some(self.expression_parser.parse_expression(tokens, pos)?)
        } else {
            None
        };

        // Parse ORDER BY clause if present
        let order_by = if self.matches_at(tokens, *pos, &Token::Order) {
            *pos += 1;
            self.expect_token(tokens, pos, &Token::By)?;
            Some(self.parse_order_by_list(tokens, pos)?)
        } else {
            None
        };

        // Parse LIMIT clause if present
        let limit = if self.matches_at(tokens, *pos, &Token::Limit) {
            *pos += 1;
            let count = Some(self.expression_parser.parse_expression(tokens, pos)?);
            let with_ties = if self.matches_at(tokens, *pos, &Token::With) {
                *pos += 1;
                // Check for "TIES" - we'll approximate this check
                if let Token::Identifier(ref s) = tokens.get(*pos).unwrap_or(&Token::Eof) {
                    if s.to_uppercase() == "TIES" {
                        *pos += 1;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };
            Some(LimitClause { count, with_ties })
        } else {
            None
        };

        // Parse OFFSET clause if present
        let offset = if self.matches_at(tokens, *pos, &Token::Offset) {
            *pos += 1;
            if let Ok(expr) = self.expression_parser.parse_expression(tokens, pos) {
                if let Expression::Literal(crate::protocols::postgres_wire::sql::types::SqlValue::Integer(n)) =
                    expr
                {
                    Some(n as u64)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok(SelectStatement {
            with,
            select_list,
            distinct,
            from_clause,
            where_clause,
            group_by,
            having,
            order_by,
            limit,
            offset,
            for_clause: None, // TODO: Parse FOR UPDATE/SHARE
        })
    }

    // Helper methods for parsing
    fn matches_at(&self, tokens: &[Token], pos: usize, expected: &Token) -> bool {
        tokens.get(pos).map_or(false, |token| {
            std::mem::discriminant(token) == std::mem::discriminant(expected)
        })
    }

    fn expect_token(
        &self,
        tokens: &[Token],
        pos: &mut usize,
        expected: &Token,
    ) -> ProtocolResult<()> {
        if self.matches_at(tokens, *pos, expected) {
            *pos += 1;
            Ok(())
        } else {
            Err(crate::protocols::error::ProtocolError::ParseError(format!(
                "Expected {:?}, found {:?}",
                expected,
                tokens.get(*pos)
            ))
            .into())
        }
    }

    fn parse_with_clause(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<WithClause> {
        self.expect_token(tokens, pos, &Token::With)?;

        let recursive =
            if self.matches_at(tokens, *pos, &Token::Identifier("RECURSIVE".to_string())) {
                *pos += 1;
                true
            } else {
                false
            };

        let mut ctes = Vec::new();
        loop {
            ctes.push(self.parse_cte(tokens, pos)?);

            if self.matches_at(tokens, *pos, &Token::Comma) {
                *pos += 1;
            } else {
                break;
            }
        }

        Ok(WithClause { recursive, ctes })
    }

    fn parse_cte(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<CommonTableExpression> {
        let name = if let Token::Identifier(n) = tokens.get(*pos).ok_or_else(|| {
            crate::protocols::error::ProtocolError::ParseError("Expected CTE name".to_string())
        })? {
            *pos += 1;
            n.clone()
        } else {
            return Err(
                crate::protocols::error::ProtocolError::ParseError("Expected CTE name".to_string()).into(),
            );
        };

        let columns = if self.matches_at(tokens, *pos, &Token::LeftParen) {
            *pos += 1;
            let cols = self.parse_identifier_list(tokens, pos)?;
            self.expect_token(tokens, pos, &Token::RightParen)?;
            Some(cols)
        } else {
            None
        };

        // Expect AS
        self.expect_token(tokens, pos, &Token::As)?;
        self.expect_token(tokens, pos, &Token::LeftParen)?;

        let query = Box::new(self.parse_select(tokens, pos)?);

        self.expect_token(tokens, pos, &Token::RightParen)?;

        Ok(CommonTableExpression {
            name,
            columns,
            query,
        })
    }

    fn parse_identifier_list(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Vec<String>> {
        let mut identifiers = Vec::new();

        loop {
            if let Token::Identifier(name) = tokens.get(*pos).ok_or_else(|| {
                crate::protocols::error::ProtocolError::ParseError("Expected identifier".to_string())
            })? {
                identifiers.push(name.clone());
                *pos += 1;
            } else {
                break;
            }

            if self.matches_at(tokens, *pos, &Token::Comma) {
                *pos += 1;
            } else {
                break;
            }
        }

        Ok(identifiers)
    }

    fn parse_select_list(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Vec<SelectItem>> {
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
                // Check for qualified wildcard (table.*)
                if *pos + 2 < tokens.len()
                    && matches!(tokens[*pos + 1], Token::Dot)
                    && matches!(tokens[*pos + 2], Token::Multiply)
                {
                    let qualifier = name.clone();
                    *pos += 3; // consume table, dot, asterisk
                    select_items.push(SelectItem::QualifiedWildcard { qualifier });
                } else {
                    // Parse as expression (could be column, function, etc.)
                    let expr = self.expression_parser.parse_expression(tokens, pos)?;

                    // Check for alias
                    let alias = self.parse_alias(tokens, pos)?;

                    select_items.push(SelectItem::Expression { expr, alias });
                }
            } else {
                // Parse complex expression
                let expr = self.expression_parser.parse_expression(tokens, pos)?;
                let alias = self.parse_alias(tokens, pos)?;
                select_items.push(SelectItem::Expression { expr, alias });
            }

            // Check for comma
            if *pos < tokens.len() && matches!(tokens[*pos], Token::Comma) {
                *pos += 1;
            } else {
                break;
            }
        }

        if select_items.is_empty() {
            return Err(crate::protocols::error::ProtocolError::ParseError(
                "Expected SELECT list".to_string(),
            )
            .into());
        }

        Ok(select_items)
    }

    fn parse_alias(&mut self, tokens: &[Token], pos: &mut usize) -> ProtocolResult<Option<String>> {
        if self.matches_at(tokens, *pos, &Token::As) {
            *pos += 1;
        }

        if let Some(Token::Identifier(alias_name)) = tokens.get(*pos) {
            *pos += 1;
            Ok(Some(alias_name.clone()))
        } else {
            Ok(None)
        }
    }

    fn parse_from_clause(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<FromClause> {
        let left = self.parse_table_reference(tokens, pos)?;

        // Check for JOINs
        if self.is_join_keyword(tokens, *pos) {
            self.parse_join(tokens, pos, left)
        } else {
            Ok(left)
        }
    }

    fn parse_table_reference(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<FromClause> {
        if self.matches_at(tokens, *pos, &Token::LeftParen) {
            *pos += 1;

            // Could be subquery or table function
            if self.matches_at(tokens, *pos, &Token::Select) {
                let query = Box::new(self.parse_select(tokens, pos)?);
                self.expect_token(tokens, pos, &Token::RightParen)?;
                let alias = self.parse_table_alias(tokens, pos)?;
                Ok(FromClause::Subquery {
                    query,
                    alias: alias.unwrap_or_else(|| TableAlias {
                        name: "".to_string(),
                        columns: None,
                    }),
                })
            } else {
                // Table function or nested table reference
                let expr = self.expression_parser.parse_expression(tokens, pos)?;
                self.expect_token(tokens, pos, &Token::RightParen)?;
                let alias = self.parse_table_alias(tokens, pos)?;

                if let Expression::Function(func) = expr {
                    Ok(FromClause::TableFunction {
                        function: *func,
                        alias,
                    })
                } else {
                    Err(crate::protocols::error::ProtocolError::ParseError(
                        "Expected table function in parentheses".to_string(),
                    )
                    .into())
                }
            }
        } else if let Token::Identifier(table_name) = tokens.get(*pos).ok_or_else(|| {
            crate::protocols::error::ProtocolError::ParseError("Expected table name".to_string())
        })? {
            let mut schema = None;
            let mut name = table_name.clone();
            *pos += 1;

            // Check for schema.table
            if self.matches_at(tokens, *pos, &Token::Dot) {
                *pos += 1;
                if let Token::Identifier(table_part) = tokens.get(*pos).ok_or_else(|| {
                    crate::protocols::error::ProtocolError::ParseError(
                        "Expected table name after schema".to_string(),
                    )
                })? {
                    schema = Some(name);
                    name = table_part.clone();
                    *pos += 1;
                }
            }

            let table = TableName { schema, name };
            let alias = self.parse_table_alias(tokens, pos)?;

            Ok(FromClause::Table { name: table, alias })
        } else {
            Err(
                crate::protocols::error::ProtocolError::ParseError("Expected table reference".to_string())
                    .into(),
            )
        }
    }

    fn parse_table_alias(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Option<TableAlias>> {
        if self.matches_at(tokens, *pos, &Token::As) {
            *pos += 1;
        }

        if let Some(Token::Identifier(alias_name)) = tokens.get(*pos) {
            let name = alias_name.clone();
            *pos += 1;

            let columns = if self.matches_at(tokens, *pos, &Token::LeftParen) {
                *pos += 1;
                let cols = self.parse_identifier_list(tokens, pos)?;
                self.expect_token(tokens, pos, &Token::RightParen)?;
                Some(cols)
            } else {
                None
            };

            Ok(Some(TableAlias { name, columns }))
        } else {
            Ok(None)
        }
    }

    fn is_join_keyword(&self, tokens: &[Token], pos: usize) -> bool {
        matches!(
            tokens.get(pos),
            Some(Token::Join)
                | Some(Token::Inner)
                | Some(Token::Left)
                | Some(Token::Right)
                | Some(Token::Full)
                | Some(Token::Cross)
                | Some(Token::Natural)
        )
    }

    fn parse_join(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
        left: FromClause,
    ) -> ProtocolResult<FromClause> {
        let mut current_left = left;

        while self.is_join_keyword(tokens, *pos) {
            let (join_type, natural) = self.parse_join_type(tokens, pos)?;
            let right = self.parse_table_reference(tokens, pos)?;
            let condition = if natural {
                JoinCondition::Natural
            } else {
                self.parse_join_condition(tokens, pos)?
            };

            current_left = FromClause::Join {
                left: Box::new(current_left),
                join_type,
                right: Box::new(right),
                condition,
            };
        }

        Ok(current_left)
    }

    fn parse_join_type(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<(JoinType, bool)> {
        let natural = if self.matches_at(tokens, *pos, &Token::Natural) {
            *pos += 1;
            true
        } else {
            false
        };

        let join_type = if self.matches_at(tokens, *pos, &Token::Inner) {
            *pos += 1;
            self.expect_token(tokens, pos, &Token::Join)?;
            JoinType::Inner
        } else if self.matches_at(tokens, *pos, &Token::Left) {
            *pos += 1;
            if self.matches_at(tokens, *pos, &Token::Outer) {
                *pos += 1;
            }
            self.expect_token(tokens, pos, &Token::Join)?;
            JoinType::LeftOuter
        } else if self.matches_at(tokens, *pos, &Token::Right) {
            *pos += 1;
            if self.matches_at(tokens, *pos, &Token::Outer) {
                *pos += 1;
            }
            self.expect_token(tokens, pos, &Token::Join)?;
            JoinType::RightOuter
        } else if self.matches_at(tokens, *pos, &Token::Full) {
            *pos += 1;
            if self.matches_at(tokens, *pos, &Token::Outer) {
                *pos += 1;
            }
            self.expect_token(tokens, pos, &Token::Join)?;
            JoinType::FullOuter
        } else if self.matches_at(tokens, *pos, &Token::Cross) {
            *pos += 1;
            self.expect_token(tokens, pos, &Token::Join)?;
            JoinType::Cross
        } else {
            // Just JOIN defaults to INNER
            self.expect_token(tokens, pos, &Token::Join)?;
            JoinType::Inner
        };

        Ok((join_type, natural))
    }

    fn parse_join_condition(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<JoinCondition> {
        if self.matches_at(tokens, *pos, &Token::On) {
            *pos += 1;
            let expr = self.expression_parser.parse_expression(tokens, pos)?;
            Ok(JoinCondition::On(expr))
        } else if self.matches_at(tokens, *pos, &Token::Using) {
            *pos += 1;
            self.expect_token(tokens, pos, &Token::LeftParen)?;
            let columns = self.parse_identifier_list(tokens, pos)?;
            self.expect_token(tokens, pos, &Token::RightParen)?;
            Ok(JoinCondition::Using(columns))
        } else {
            Err(crate::protocols::error::ProtocolError::ParseError(
                "Expected ON or USING in join condition".to_string(),
            )
            .into())
        }
    }

    fn parse_expression_list(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Vec<Expression>> {
        let mut expressions = Vec::new();

        loop {
            expressions.push(self.expression_parser.parse_expression(tokens, pos)?);

            if self.matches_at(tokens, *pos, &Token::Comma) {
                *pos += 1;
            } else {
                break;
            }
        }

        Ok(expressions)
    }

    fn parse_order_by_list(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<Vec<OrderByItem>> {
        let mut items = Vec::new();

        loop {
            let expression = self.expression_parser.parse_expression(tokens, pos)?;

            let direction = if self.matches_at(tokens, *pos, &Token::Asc) {
                *pos += 1;
                Some(SortDirection::Ascending)
            } else if self.matches_at(tokens, *pos, &Token::Desc) {
                *pos += 1;
                Some(SortDirection::Descending)
            } else {
                None
            };

            let nulls = if self.matches_at(tokens, *pos, &Token::Nulls) {
                *pos += 1;
                if self.matches_at(tokens, *pos, &Token::First) {
                    *pos += 1;
                    Some(NullsOrder::First)
                } else if self.matches_at(tokens, *pos, &Token::Last) {
                    *pos += 1;
                    Some(NullsOrder::Last)
                } else {
                    None
                }
            } else {
                None
            };

            items.push(OrderByItem {
                expression,
                direction,
                nulls,
            });

            if self.matches_at(tokens, *pos, &Token::Comma) {
                *pos += 1;
            } else {
                break;
            }
        }

        Ok(items)
    }
}

impl Default for SelectParser {
    fn default() -> Self {
        Self::new()
    }
}
