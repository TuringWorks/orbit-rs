//! Example demonstrating AQL parser functionality

use orbit_protocols::aql::aql_parser::AqlParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing AQL Parser");

    let parser = AqlParser::new();

    // Test basic FOR query
    let query1 = "FOR doc IN users RETURN doc";
    match parser.parse(query1) {
        Ok(parsed) => {
            println!("✅ Successfully parsed query 1: {:?}", parsed);
            println!("   Clauses: {}", parsed.clauses.len());
        }
        Err(e) => {
            println!("❌ Failed to parse query 1: {:?}", e);
        }
    }

    // Test FILTER query
    let query2 = "FOR doc IN users FILTER doc.age > 25 RETURN doc";
    match parser.parse(query2) {
        Ok(parsed) => {
            println!("✅ Successfully parsed query 2: {:?}", parsed);
            println!("   Clauses: {}", parsed.clauses.len());
        }
        Err(e) => {
            println!("❌ Failed to parse query 2: {:?}", e);
        }
    }

    // Test graph traversal
    let query3 =
        "FOR vertex, edge, path IN 1..3 OUTBOUND 'users/john' GRAPH 'social' RETURN vertex";
    match parser.parse(query3) {
        Ok(parsed) => {
            println!("✅ Successfully parsed query 3: {:?}", parsed);
            println!("   Clauses: {}", parsed.clauses.len());
        }
        Err(e) => {
            println!("❌ Failed to parse query 3: {:?}", e);
        }
    }

    // Test INSERT query
    let query4 = "INSERT {name: 'Alice', age: 30} INTO users";
    match parser.parse(query4) {
        Ok(parsed) => {
            println!("✅ Successfully parsed query 4: {:?}", parsed);
            println!("   Clauses: {}", parsed.clauses.len());
        }
        Err(e) => {
            println!("❌ Failed to parse query 4: {:?}", e);
        }
    }

    // Test invalid query
    let query5 = "INVALID SYNTAX HERE";
    match parser.parse(query5) {
        Ok(_) => {
            println!("❌ Should have failed to parse invalid query");
        }
        Err(_) => {
            println!("✅ Correctly failed to parse invalid query");
        }
    }

    println!("\nAQL Parser test completed!");
    Ok(())
}
