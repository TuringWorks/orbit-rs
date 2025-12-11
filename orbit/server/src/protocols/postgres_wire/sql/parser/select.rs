//! SELECT Statement Parser Implementation

#![allow(clippy::useless_conversion)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::while_let_loop)]
#![allow(clippy::unnecessary_map_or)]

use crate::protocols::error::ProtocolResult;
use crate::protocols::postgres_wire::sql::ast::{
    CommonTableExpression, DistinctClause, Expression, FromClause, JoinCondition, JoinType,
    LimitClause, NullsOrder, OrderByItem, SelectItem, SelectStatement, SortDirection, TableAlias,
    TableName, WithClause, SetOperation, SetOperator,
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
        // Parse WITH clause if present
        let with = if *pos < tokens.len() && matches!(tokens[*pos], Token::With) {
            Some(self.parse_with_clause(tokens, pos)?)
        } else {
            None
        };

        // Expect SELECT keyword
        if *pos >= tokens.len() || !matches!(tokens[*pos], Token::Select) {
            return Err(crate::protocols::error::ProtocolError::ParseError(
                "Expected SELECT".to_string(),
            )
            .into());
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
                if let Expression::Literal(
                    crate::protocols::postgres_wire::sql::types::SqlValue::Integer(n),
                ) = expr
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
            traverse: None,
            set_operation: {
                if *pos < tokens.len() {
                    let token = &tokens[*pos];
                    if matches!(token, Token::Union | Token::Intersect | Token::Except) {
                         Some(self.parse_set_operation(tokens, pos)?)
                    } else {
                        None
                    }
                } else {
                    None
                }
            },
        })
    }

    // Helper methods for parsing
    fn matches_at(&self, tokens: &[Token], pos: usize, expected: &Token) -> bool {
        tokens.get(pos).map_or(false, |token| token == expected)
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

    pub fn parse_with_clause(
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
        let name = if let Some(n) = tokens.get(*pos).and_then(crate::protocols::postgres_wire::sql::parser::utilities::token_to_identifier_name) {
            *pos += 1;
            n
        } else {
            return Err(crate::protocols::error::ProtocolError::ParseError(
                "Expected CTE name".to_string(),
            )
            .into());
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
                crate::protocols::error::ProtocolError::ParseError(
                    "Expected identifier".to_string(),
                )
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
            } else if matches!(tokens[*pos], Token::Old | Token::New) {
                // PostgreSQL 18 - OLD.* and NEW.* in RETURNING clause
                let qualifier = match &tokens[*pos] {
                    Token::Old => "OLD".to_string(),
                    Token::New => "NEW".to_string(),
                    _ => unreachable!(),
                };

                // Check for qualified wildcard (OLD.* or NEW.*)
                if *pos + 2 < tokens.len()
                    && matches!(tokens[*pos + 1], Token::Dot)
                    && matches!(tokens[*pos + 2], Token::Multiply)
                {
                    *pos += 3; // consume OLD/NEW, dot, asterisk
                    select_items.push(SelectItem::QualifiedWildcard { qualifier });
                } else {
                    // Parse as expression (OLD.column or NEW.column)
                    let expr = self.expression_parser.parse_expression(tokens, pos)?;
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

        if let Some(alias_name) = tokens.get(*pos).and_then(crate::protocols::postgres_wire::sql::parser::utilities::token_to_identifier_name) {
            *pos += 1;
            Ok(Some(alias_name))
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
        let lateral = if self.matches_at(tokens, *pos, &Token::Lateral) {
            *pos += 1;
            true
        } else {
            false
        };

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
                    lateral,
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
                        lateral,
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
            // Check for JSON_TABLE
            if table_name.to_uppercase() == "JSON_TABLE"
                && self.matches_at(tokens, *pos + 1, &Token::LeftParen)
            {
                if lateral {
                     return Err(crate::protocols::error::ProtocolError::ParseError(
                        "LATERAL not yet supported for JSON_TABLE".to_string(),
                    ).into());
                }
                return self.parse_json_table(tokens, pos);
            }

            if lateral {
                return Err(crate::protocols::error::ProtocolError::ParseError(
                    "LATERAL can only be used with subqueries or function calls".to_string(),
                )
                .into());
            }

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
            Err(crate::protocols::error::ProtocolError::ParseError(
                "Expected table reference".to_string(),
            )
            .into())
        }
    }

    fn parse_json_table(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<FromClause> {
        *pos += 1; // consume JSON_TABLE
        self.expect_token(tokens, pos, &Token::LeftParen)?;

        // Parse context item (JSON document)
        let context_item = self.expression_parser.parse_expression(tokens, pos)?;
        self.expect_token(tokens, pos, &Token::Comma)?;

        // Parse path expression
        let path_expression = self.expression_parser.parse_expression(tokens, pos)?;

        // Parse COLUMNS clause
        // Expected syntax: COLUMNS ( name type [PATH path] ... )
        // Note: The test query has 'COLUMNS' directly after path expression without comma
        // SELECT ... json_table(..., '$[*]' COLUMNS ...)

        // Check for optional comma before COLUMNS (standard SQL might require it, but test query doesn't seem to use it?)
        // Actually standard SQL is: JSON_TABLE(context, path COLUMNS ...)
        // But let's handle optional comma just in case
        if self.matches_at(tokens, *pos, &Token::Comma) {
            *pos += 1;
        }

        let mut columns = Vec::new();
        if let Token::Identifier(s) = tokens.get(*pos).ok_or_else(|| {
            crate::protocols::error::ProtocolError::ParseError(
                "Expected COLUMNS keyword".to_string(),
            )
        })? {
            if s.to_uppercase() == "COLUMNS" {
                *pos += 1;
                self.expect_token(tokens, pos, &Token::LeftParen)?;

                loop {
                    // Parse column definition: name type [PATH path]
                    let name = if let Token::Identifier(n) = tokens.get(*pos).ok_or_else(|| {
                        crate::protocols::error::ProtocolError::ParseError(
                            "Expected column name".to_string(),
                        )
                    })? {
                        n.clone()
                    } else {
                        break;
                    };
                    *pos += 1;

                    let data_type = self.expression_parser.parse_sql_type(tokens, pos)?;

                    let mut path = None;
                    if let Token::Identifier(p) = tokens.get(*pos).unwrap_or(&Token::Eof) {
                        if p.to_uppercase() == "PATH" {
                            *pos += 1;
                            if let Token::StringLiteral(path_str) =
                                tokens.get(*pos).ok_or_else(|| {
                                    crate::protocols::error::ProtocolError::ParseError(
                                        "Expected path string literal".to_string(),
                                    )
                                })?
                            {
                                path = Some(path_str.clone());
                                *pos += 1;
                            }
                        }
                    }

                    columns.push(crate::protocols::postgres_wire::sql::ast::JsonTableColumn {
                        name,
                        data_type,
                        path,
                    });

                    if self.matches_at(tokens, *pos, &Token::Comma) {
                        *pos += 1;
                    } else {
                        break;
                    }
                }

                self.expect_token(tokens, pos, &Token::RightParen)?;
            }
        }

        self.expect_token(tokens, pos, &Token::RightParen)?;

        let alias = self.parse_table_alias(tokens, pos)?;

        Ok(FromClause::JsonTable(
            crate::protocols::postgres_wire::sql::ast::JsonTable {
                context_item,
                path_expression,
                columns,
                alias,
            },
        ))
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

    fn parse_set_operation(
        &mut self,
        tokens: &[Token],
        pos: &mut usize,
    ) -> ProtocolResult<SetOperation> {
        // Determine the operator
        let operator = if *pos < tokens.len() {
            match &tokens[*pos] {
                Token::Union => {
                    *pos += 1;
                    if self.matches_at(tokens, *pos, &Token::All) {
                        *pos += 1;
                        SetOperator::UnionAll
                    } else if self.matches_at(tokens, *pos, &Token::Distinct) {
                        *pos += 1;
                        SetOperator::Union
                    } else {
                        SetOperator::Union
                    }
                }
                Token::Intersect => {
                    *pos += 1;
                    if self.matches_at(tokens, *pos, &Token::All) {
                        *pos += 1;
                        SetOperator::IntersectAll
                    } else if self.matches_at(tokens, *pos, &Token::Distinct) {
                        *pos += 1;
                        SetOperator::Intersect
                    } else {
                        SetOperator::Intersect
                    }
                }
                Token::Except => {
                    *pos += 1;
                    if self.matches_at(tokens, *pos, &Token::All) {
                        *pos += 1;
                        SetOperator::ExceptAll
                    } else if self.matches_at(tokens, *pos, &Token::Distinct) {
                        *pos += 1;
                        SetOperator::Except
                    } else {
                        SetOperator::Except
                    }
                }
                _ => return Err(crate::protocols::error::ProtocolError::ParseError(
                     "Expected UNION, INTERSECT, or EXCEPT".to_string()
                ).into())
            }
        } else {
             return Err(crate::protocols::error::ProtocolError::ParseError(
                 "Unexpected end of input".to_string()
            ).into())
        };

        // Parse right side
        // Right side must be a SELECT statement
        // Note: We need to consume SELECT keyword if parse_select expects it.
        // Based on logic, parse_select DOES expect SELECT (checked in next step).
        // But if parse_select handles WITH, it might check WITH first.
        // Recursive union typically: ... UNION ALL SELECT ...
        // So SELECT is present.
        
        let right = self.parse_select(tokens, pos)?;
        
        Ok(SetOperation {
            operator,
            right: Box::new(right),
        })
    }
}

impl Default for SelectParser {
    fn default() -> Self {
        Self::new()
    }
}
