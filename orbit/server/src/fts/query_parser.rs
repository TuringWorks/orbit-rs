// Query Parser
//
// Parses protocol-specific query syntax to Tantivy queries.

use anyhow::{anyhow, Result};
use tantivy::query::*;
use tantivy::schema::*;
use tantivy::Index;

/// Query parser for different protocols
pub struct QueryParser {
    index: Index,
    default_fields: Vec<Field>,
}

impl QueryParser {
    pub fn new(index: Index, default_fields: Vec<Field>) -> Self {
        Self {
            index,
            default_fields,
        }
    }

    /// Parse PostgreSQL tsquery to Tantivy query
    /// Syntax: 'cat & dog' → AND, 'cat | dog' → OR, '!cat' → NOT
    pub fn parse_tsquery(&self, tsquery: &str) -> Result<Box<dyn Query>> {
        let query_parser =
            tantivy::query::QueryParser::for_index(&self.index, self.default_fields.clone());

        // Convert PostgreSQL operators to Tantivy syntax
        let tantivy_query = tsquery
            .replace(" & ", " AND ")
            .replace(" | ", " OR ")
            .replace("!", "NOT ");

        query_parser
            .parse_query(&tantivy_query)
            .map_err(|e| anyhow!("Failed to parse tsquery: {}", e))
    }

    /// Parse MySQL boolean mode query to Tantivy query
    /// Syntax: '+must +have -exclude' → Boolean query
    pub fn parse_mysql_boolean(&self, query: &str) -> Result<Box<dyn Query>> {
        let mut must = Vec::new();
        let mut must_not = Vec::new();
        let mut should = Vec::new();

        let query_parser =
            tantivy::query::QueryParser::for_index(&self.index, self.default_fields.clone());

        for term in query.split_whitespace() {
            if term.starts_with('+') {
                let word = &term[1..];
                must.push(query_parser.parse_query(word)?);
            } else if term.starts_with('-') {
                let word = &term[1..];
                must_not.push(query_parser.parse_query(word)?);
            } else {
                should.push(query_parser.parse_query(term)?);
            }
        }

        // Build boolean query using clauses
        let mut clauses = Vec::new();
        for q in must {
            clauses.push((Occur::Must, q));
        }
        for q in must_not {
            clauses.push((Occur::MustNot, q));
        }
        for q in should {
            clauses.push((Occur::Should, q));
        }

        Ok(Box::new(BooleanQuery::from(clauses)))
    }

    /// Parse MongoDB $text query to Tantivy query
    pub fn parse_mongodb_text(&self, search: &str) -> Result<Box<dyn Query>> {
        let query_parser =
            tantivy::query::QueryParser::for_index(&self.index, self.default_fields.clone());

        query_parser
            .parse_query(search)
            .map_err(|e| anyhow!("Failed to parse MongoDB text query: {}", e))
    }

    /// Parse Redis FT.SEARCH query to Tantivy query
    pub fn parse_redis_search(&self, query: &str) -> Result<Box<dyn Query>> {
        let query_parser =
            tantivy::query::QueryParser::for_index(&self.index, self.default_fields.clone());

        query_parser
            .parse_query(query)
            .map_err(|e| anyhow!("Failed to parse Redis search query: {}", e))
    }

    /// Parse standard query (Lucene-like syntax)
    pub fn parse_standard(&self, query: &str) -> Result<Box<dyn Query>> {
        let query_parser =
            tantivy::query::QueryParser::for_index(&self.index, self.default_fields.clone());

        query_parser
            .parse_query(query)
            .map_err(|e| anyhow!("Failed to parse query: {}", e))
    }

    /// Parse fuzzy query
    pub fn parse_fuzzy(&self, term: &str, distance: u8) -> Result<Box<dyn Query>> {
        if self.default_fields.is_empty() {
            return Err(anyhow!("No default fields specified"));
        }

        let field = self.default_fields[0];
        let term_obj = Term::from_field_text(field, term);

        Ok(Box::new(FuzzyTermQuery::new(term_obj, distance, true)))
    }

    /// Parse phrase query
    pub fn parse_phrase(&self, phrase: &str) -> Result<Box<dyn Query>> {
        if self.default_fields.is_empty() {
            return Err(anyhow!("No default fields specified"));
        }

        let field = self.default_fields[0];
        let terms: Vec<Term> = phrase
            .split_whitespace()
            .map(|word| Term::from_field_text(field, word))
            .collect();

        Ok(Box::new(PhraseQuery::new(terms)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tantivy::Index;

    #[test]
    fn test_parse_tsquery() {
        let mut schema_builder = Schema::builder();
        let title = schema_builder.add_text_field("title", TEXT);
        let schema = schema_builder.build();

        let index = Index::create_in_ram(schema);
        let parser = QueryParser::new(index, vec![title]);

        let query = parser.parse_tsquery("cat & dog").unwrap();
        // Tantivy uses BooleanQuery for AND operations
        let query_debug = format!("{:?}", query);
        assert!(
            query_debug.contains("Boolean")
                || query_debug.contains("cat") && query_debug.contains("dog"),
            "Query should contain both terms: {}",
            query_debug
        );
        assert!(format!("{:?}", query).contains("AND"));
    }
}
