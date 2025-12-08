// FTS Schema Definitions
//
// Provides schema builders for different protocols.

use tantivy::schema::*;

/// Schema builder for PostgreSQL FTS
pub struct PostgresSchema;

impl PostgresSchema {
    /// Create schema for PostgreSQL tsvector index
    pub fn build(fields: &[&str]) -> Schema {
        let mut schema_builder = Schema::builder();

        // Add document ID field
        schema_builder.add_text_field("_id", STRING | STORED);

        // Add text fields with full-text indexing
        for field_name in fields {
            let text_options = TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("en_stem")
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                )
                .set_stored();

            schema_builder.add_text_field(field_name, text_options);
        }

        schema_builder.build()
    }
}

/// Schema builder for MySQL FULLTEXT index
pub struct MysqlSchema;

impl MysqlSchema {
    /// Create schema for MySQL FULLTEXT index
    pub fn build(fields: &[&str]) -> Schema {
        let mut schema_builder = Schema::builder();

        // Add document ID field
        schema_builder.add_text_field("_id", STRING | STORED);

        // Add text fields
        for field_name in fields {
            let text_options = TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("default")
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                )
                .set_stored();

            schema_builder.add_text_field(field_name, text_options);
        }

        schema_builder.build()
    }
}

/// Schema builder for MongoDB text index
pub struct MongodbSchema;

impl MongodbSchema {
    /// Create schema for MongoDB text index
    pub fn build(fields: &[(&str, i32)]) -> Schema {
        let mut schema_builder = Schema::builder();

        // Add document ID field
        schema_builder.add_text_field("_id", STRING | STORED);

        // Add text fields with weights
        for (field_name, _weight) in fields {
            let text_options = TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("default")
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                )
                .set_stored();

            schema_builder.add_text_field(field_name, text_options);
        }

        schema_builder.build()
    }
}

/// Schema builder for Redis FT.CREATE
pub struct RedisSchema;

impl RedisSchema {
    /// Create schema for Redis search index
    pub fn build(fields: &[(&str, &str)]) -> Schema {
        let mut schema_builder = Schema::builder();

        // Add document ID field
        schema_builder.add_text_field("_id", STRING | STORED);

        // Add fields based on type
        for (field_name, field_type) in fields {
            match *field_type {
                "TEXT" => {
                    let text_options = TextOptions::default()
                        .set_indexing_options(
                            TextFieldIndexing::default()
                                .set_tokenizer("default")
                                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                        )
                        .set_stored();

                    schema_builder.add_text_field(field_name, text_options);
                }
                "TAG" => {
                    schema_builder.add_text_field(field_name, STRING | STORED);
                }
                "NUMERIC" => {
                    schema_builder.add_i64_field(field_name, INDEXED | STORED);
                }
                _ => {
                    // Default to text
                    schema_builder.add_text_field(field_name, TEXT | STORED);
                }
            }
        }

        schema_builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postgres_schema() {
        let schema = PostgresSchema::build(&["title", "body"]);
        assert_eq!(schema.fields().count(), 3); // _id + title + body
    }

    #[test]
    fn test_mysql_schema() {
        let schema = MysqlSchema::build(&["title", "content"]);
        assert_eq!(schema.fields().count(), 3);
    }

    #[test]
    fn test_mongodb_schema() {
        let schema = MongodbSchema::build(&[("title", 10), ("body", 1)]);
        assert_eq!(schema.fields().count(), 3);
    }

    #[test]
    fn test_redis_schema() {
        let schema = RedisSchema::build(&[
            ("title", "TEXT"),
            ("price", "NUMERIC"),
            ("tags", "TAG"),
        ]);
        assert_eq!(schema.fields().count(), 4);
    }
}
