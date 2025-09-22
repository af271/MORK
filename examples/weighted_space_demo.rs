use std::io::Write;
use mork_bytestring::Tag;
use mork::weighted_space::{WeightedSpace, WeightedQueryResult};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 MORK WeightedSpace Demo");
    println!("==========================");

    // Create a new weighted space
    let mut space = WeightedSpace::new_with_k(10);

    // Create some sample expressions
    let expressions = vec![
        // (+ a b) - appears 5 times
        create_expression(&["Arity", "2", "SymbolSize", "1", "+", "SymbolSize", "1", "a", "SymbolSize", "1", "b"], 5),
        // (+ a c) - appears 3 times  
        create_expression(&["Arity", "2", "SymbolSize", "1", "+", "SymbolSize", "1", "a", "SymbolSize", "1", "c"], 3),
        // (* x y) - appears 8 times
        create_expression(&["Arity", "2", "SymbolSize", "1", "*", "SymbolSize", "1", "x", "SymbolSize", "1", "y"], 8),
        // (= foo bar) - appears 2 times
        create_expression(&["Arity", "2", "SymbolSize", "1", "=", "SymbolSize", "3", "foo", "SymbolSize", "3", "bar"], 2),
        // (lambda x (+ x 1)) - appears 1 time
        create_expression(&["Arity", "2", "SymbolSize", "6", "lambda", "SymbolSize", "1", "x", "Arity", "2", "SymbolSize", "1", "+", "SymbolSize", "1", "x", "SymbolSize", "1", "1"], 1),
    ];

    println!("\n📝 Adding expressions to weighted space:");
    for (expr_bytes, count, description) in &expressions {
        println!("  Adding: {} (count: {})", description, count);
        space.add_expression_weighted(expr_bytes, *count)?;
    }

    println!("\n📊 Top expressions by frequency:");
    let top_expressions = space.get_top_k_expressions(5);
    for (i, result) in top_expressions.iter().enumerate() {
        println!("  {}. Weight: {} (local: {}, subtree: {})", 
                 i + 1, 
                 result.weight.local_count + result.weight.subtree_count,
                 result.weight.local_count,
                 result.weight.subtree_count);
        println!("     Expression bytes: {:?}", 
                 result.expression.iter().take(20).map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "));
    }

    println!("\n🔍 Demonstrating expression lookup:");
    for (expr_bytes, count, description) in &expressions {
        if let Some(weight) = space.get_expression_weight(expr_bytes) {
            println!("  {}: local_count={}, subtree_count={}", 
                     description, weight.local_count, weight.subtree_count);
        }
    }

    println!("\n⚡ Incrementing counts for existing expressions:");
    space.increment_expression_count(&expressions[0].0, 2)?; // Add 2 more to (+ a b)
    space.increment_expression_count(&expressions[2].0, 1)?; // Add 1 more to (* x y)

    println!("  Updated counts:");
    if let Some(weight) = space.get_expression_weight(&expressions[0].0) {
        println!("    (+ a b): {}", weight.local_count);
    }
    if let Some(weight) = space.get_expression_weight(&expressions[2].0) {
        println!("    (* x y): {}", weight.local_count);
    }

    println!("\n📈 Final top expressions:");
    let final_top = space.get_top_k_expressions(5);
    for (i, result) in final_top.iter().enumerate() {
        println!("  {}. Total weight: {}", 
                 i + 1, 
                 result.weight.local_count + result.weight.subtree_count);
    }

    println!("\n✅ WeightedSpace demo completed successfully!");
    Ok(())
}

fn create_expression(tokens: &[&str], count: u64) -> (Vec<u8>, u64, String) {
    let mut bytes = Vec::new();
    let mut description = String::new();
    
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "Arity" => {
                if i + 1 < tokens.len() {
                    let arity: u8 = tokens[i + 1].parse().unwrap();
                    bytes.push(Tag::Arity(arity).encode_as_byte().unwrap());
                    description.push('(');
                    i += 2;
                } else {
                    i += 1;
                }
            },
            "SymbolSize" => {
                if i + 2 < tokens.len() {
                    let size: u8 = tokens[i + 1].parse().unwrap();
                    let symbol = tokens[i + 2];
                    bytes.push(Tag::SymbolSize(size).encode_as_byte().unwrap());
                    bytes.extend_from_slice(symbol.as_bytes());
                    if !description.is_empty() && !description.ends_with('(') {
                        description.push(' ');
                    }
                    description.push_str(symbol);
                    i += 3;
                } else {
                    i += 1;
                }
            },
            "VarRef" => {
                if i + 1 < tokens.len() {
                    let var_id: u8 = tokens[i + 1].parse().unwrap();
                    bytes.push(Tag::VarRef(var_id).encode_as_byte().unwrap());
                    description.push_str(&format!("_{}", var_id));
                    i += 2;
                } else {
                    i += 1;
                }
            },
            "NewVar" => {
                bytes.push(Tag::NewVar.encode_as_byte().unwrap());
                description.push('$');
                i += 1;
            },
            _ => {
                i += 1;
            }
        }
    }
    
    // Close parentheses for arity expressions
    let arity_count = description.matches('(').count();
    for _ in 0..arity_count {
        description.push(')');
    }
    
    (bytes, count, description)
}
