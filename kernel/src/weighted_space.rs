use std::sync::{Arc, Mutex};
use std::collections::BTreeMap;
use std::ops::Deref;

use pathmap::weighted_trie::{WeightedTriemap, NodeWeight};
use mork_bytestring::{Expr, ExprEnv, ExprTrait};

use crate::space::DefaultSpace;

/// A weighted extension of DefaultSpace that tracks expression counts and weights
pub struct WeightedSpace {
    /// The original DefaultSpace for all existing operations
    pub base_space: DefaultSpace,
    /// Weight tracking trie (parallel to base_space.map)
    pub weights: Arc<Mutex<WeightedTriemap<NodeWeight>>>,
}

/// Result type for weighted queries that includes expression weights
#[derive(Debug, Clone)]
pub struct WeightedQueryResult {
    pub expression: Vec<u8>,  // The raw expression bytes
    pub weight: NodeWeight,   // Associated weight information
}

impl WeightedSpace {
    /// Creates a new empty weighted space
    pub fn new() -> Self {
        Self {
            base_space: DefaultSpace::new(),
            weights: Arc::new(Mutex::new(WeightedTriemap::new())),
        }
    }

    /// Creates a new weighted space with specified top-k tracking
    pub fn new_with_k(k: usize) -> Self {
        Self {
            base_space: DefaultSpace::new(),
            weights: Arc::new(Mutex::new(WeightedTriemap::new_with_k(k))),
        }
    }

    /// Query with weight information included in results - REAL IMPLEMENTATION
    /// This method automatically tracks weights of expressions that match the query
    pub fn query_multi_weighted<F>(&self, patterns: &[Expr], mut effect: F) -> usize 
    where 
        F: FnMut(Result<&[ExprEnv], (BTreeMap<(u8, u8), ExprEnv>, u8, u8, &[(u8, u8)])>, Expr, NodeWeight) -> bool,
    {
        let weights_ref = self.weights.clone();
        
        self.base_space.query_multi(patterns, |result, expr| {
            // Get the expression path for weight lookup/tracking
            let path = unsafe { expr.span().as_ref().unwrap() };
            
            // Automatically increment weight count for this expression since it was queried
            let weight = {
                let mut weights_guard = weights_ref.lock().unwrap();
                // Increment count by 1 each time this expression is found by a query
                let _ = weights_guard.increment_count(path, 1);
                // Return the updated weight
                weights_guard.get_weight_copy(path).unwrap_or_default()
            };
            
            // Call the extended callback with weight information
            effect(result, expr, weight)
        })
    }

    /// Regular query method that automatically tracks expression frequency
    pub fn query<F>(&self, pattern: Expr, mut effect: F) 
    where 
        F: FnMut(Expr) -> (),
    {
        self.query_multi_weighted(&[pattern], |_result, expr, _weight| {
            effect(expr);
            true // Continue with all results
        });
    }

    /// Query and collect weighted results
    pub fn query_weighted_collect(&self, patterns: &[Expr]) -> Vec<WeightedQueryResult> {
        let mut results = Vec::new();
        
        self.query_multi_weighted(patterns, |_result, expr, weight| {
            let path = unsafe { expr.span().as_ref().unwrap() };
            results.push(WeightedQueryResult {
                expression: path.to_vec(),
                weight,
            });
            true // Continue collecting all results
        });

        results
    }

    /// Query and get top-k heaviest expressions
    pub fn query_top_k_weighted(&self, patterns: &[Expr], k: usize) -> Vec<WeightedQueryResult> {
        let mut results = self.query_weighted_collect(patterns);
        
        // Sort by total weight (local_count + subtree_count) in descending order
        results.sort_by(|a, b| {
            let weight_a = a.weight.local_count + a.weight.subtree_count;
            let weight_b = b.weight.local_count + b.weight.subtree_count;
            weight_b.cmp(&weight_a)
        });

        results.truncate(k);
        results
    }

    /// Add an expression with specified weight
    pub fn add_expression_weighted(&mut self, expr: &[u8], local_count: u64) -> Result<(), String> {
        // Add to base space (using existing infrastructure)
        // Note: This would need to be implemented based on existing DefaultSpace methods
        // For now, we'll focus on the weight tracking part
        
        // Add weight information
        {
            let mut weights = self.weights.lock().unwrap();
            weights.increment_count(expr, local_count)?;
        }

        Ok(())
    }

    /// Increment count for existing expression
    pub fn increment_expression_count(&self, expr: &[u8], count: u64) -> Result<(), String> {
        let mut weights = self.weights.lock().unwrap();
        weights.increment_count(expr, count)
    }

    /// Get weight information for a specific expression
    pub fn get_expression_weight(&self, expr: &[u8]) -> Option<NodeWeight> {
        let weights = self.weights.lock().unwrap();
        weights.get_weight_copy(expr)
    }

    /// Get top-k most frequent expressions overall
    pub fn get_top_k_expressions(&self, k: usize) -> Vec<WeightedQueryResult> {
        let weights = self.weights.lock().unwrap();
        weights.get_top_k_paths(k).into_iter().map(|(path, weight)| {
            WeightedQueryResult {
                expression: path,
                weight,
            }
        }).collect()
    }

    /// Update subtree counts efficiently
    pub fn recompute_subtree_counts(&self) -> Result<(), String> {
        let mut weights = self.weights.lock().unwrap();
        weights.recompute_subtree_counts()
    }
}

// Delegate all DefaultSpace methods to base_space
impl Deref for WeightedSpace {
    type Target = DefaultSpace;
    
    fn deref(&self) -> &Self::Target {
        &self.base_space
    }
}

impl std::ops::DerefMut for WeightedSpace {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base_space
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mork_bytestring::Tag;

    // Real test data similar to lib.rs
    const REAL_SEXPRS: &str = r#"(first_name John)
(last_name Smith)
(is_alive true)
(age 27)
(address (street_address 21 2nd Street))
(address (city New York))
(address (state NY))
(address (postal_code 10021-3100))
(phone_numbers (0 (type home)))
(phone_numbers (0 (number 212 555-1234)))
(phone_numbers (1 (type office)))
(phone_numbers (1 (number 646 555-4567)))
(children (0 Catherine))
(children (1 Thomas))
(children (2 Trevor))
(spouse null)
"#;

    #[test]
    fn test_weighted_space_creation() {
        let space = WeightedSpace::new();
        assert!(space.weights.lock().unwrap().is_empty());
    }

    #[test]
    fn test_expression_weight_tracking() {
        let mut space = WeightedSpace::new();
        
        // Create a simple expression bytecode: Arity(2) + Symbol "+" + Symbol "a" + Symbol "b"
        let expr_bytes = vec![
            Tag::Arity(2).encode_as_byte().unwrap(),
            Tag::SymbolSize(1).encode_as_byte().unwrap(), b'+',
            Tag::SymbolSize(1).encode_as_byte().unwrap(), b'a',
            Tag::SymbolSize(1).encode_as_byte().unwrap(), b'b',
        ];

        // Add expression with weight
        space.add_expression_weighted(&expr_bytes, 5).unwrap();
        
        // Check weight
        let weight = space.get_expression_weight(&expr_bytes).unwrap();
        assert_eq!(weight.local_count, 5);
    }

    #[test]
    fn test_weighted_query_integration() {
        let space = WeightedSpace::new();
        
        // This test would need actual expressions in the space to be meaningful
        // For now, just verify the query method doesn't panic
        let patterns = vec![];
        let results = space.query_weighted_collect(&patterns);
        assert!(results.is_empty());
    }

    #[test]
    fn test_real_weighted_queries() {
        use crate::{expr, sexpr};
        use crate::space_temporary::Space;
        
        let mut space = WeightedSpace::new();
        
        // Load real S-expressions into the space
        let pattern_expr = expr!(space, "$");
        let metadata_expr = expr!(space, "_1");
        
        let loaded_count = space.load_sexpr_simple(
            REAL_SEXPRS.as_bytes(), 
            pattern_expr, 
            metadata_expr
        ).unwrap();
        
        println!("Loaded {} expressions", loaded_count);
        assert!(loaded_count > 0, "Should load some expressions");

        // Now perform REAL queries that will automatically track weights
        let children_pattern = expr!(space, "[2] children [2] $ $");
        
        let mut found_children = Vec::new();
        let mut match_index = 0;
        let query_count = space.query_multi_weighted(&[children_pattern], |_result, expr, weight| {
            let path = unsafe { expr.span().as_ref().unwrap() };
            let expr_str = sexpr!(space, expr);
            println!("Found child expression with weight: local={}, subtree={}", 
                     weight.local_count, weight.subtree_count);
            println!("Match {}: {}", match_index, expr_str);
            
            // Validate expected matches based on pattern structure
            match match_index {
                0 => { assert_eq!(expr_str, "((children (0 Catherine)))") }
                1 => { assert_eq!(expr_str, "((children (1 Thomas)))") }
                2 => { assert_eq!(expr_str, "((children (2 Trevor)))") }
                _ => { panic!("Unexpected additional match: {}", expr_str) }
            }
            
            found_children.push((path.to_vec(), weight, expr_str.clone()));
            match_index += 1;
            true // Continue collecting all results
        });

        println!("Query found {} children expressions", query_count);
        assert!(query_count > 0, "Should find some children expressions");
        
        // Query again - weights should have increased because expressions were queried again
        let mut found_children_second = Vec::new();
        let mut second_match_index = 0;
        let second_query_count = space.query_multi_weighted(&[children_pattern], |_result, expr, weight| {
            let path = unsafe { expr.span().as_ref().unwrap() };
            let expr_str = sexpr!(space, expr);
            println!("Second query - child expression with weight: local={}, subtree={}", 
                     weight.local_count, weight.subtree_count);
            println!("Second match {}: {}", second_match_index, expr_str);
            
            found_children_second.push((path.to_vec(), weight, expr_str.clone()));
            second_match_index += 1;
            true
        });

        assert_eq!(second_query_count, query_count, "Should find same number of expressions");
        
        // Verify that weights increased between first and second query
        for (i, ((path1, weight1, expr1), (path2, weight2, expr2))) in 
            found_children.iter().zip(found_children_second.iter()).enumerate() {
            assert_eq!(path1, path2, "Paths should be identical");
            assert_eq!(expr1, expr2, "Expressions should be identical");
            assert!(weight2.local_count > weight1.local_count, 
                    "Weight {} should have increased from {} to {} after second query", 
                    i, weight1.local_count, weight2.local_count);
            println!("✓ Match {}: '{}' weight increased from {} to {}", 
                     i, expr1, weight1.local_count, weight2.local_count);
        }

        // Test top-k functionality with real data
        let top_expressions = space.get_top_k_expressions(3);
        println!("Top {} expressions by weight:", top_expressions.len());
        for (i, result) in top_expressions.iter().enumerate() {
            println!("  {}. Weight: {} (local: {}, subtree: {})", 
                     i + 1,
                     result.weight.local_count + result.weight.subtree_count,
                     result.weight.local_count,
                     result.weight.subtree_count);
        }
    }

    #[test]
    fn test_query_method_weight_tracking() {
        use crate::{expr, sexpr};
        use crate::space_temporary::Space;
        
        let mut space = WeightedSpace::new();
        
        // Load expressions
        let pattern_expr = expr!(space, "$");
        let metadata_expr = expr!(space, "_1"); 
        space.load_sexpr_simple(
            REAL_SEXPRS.as_bytes(), 
            pattern_expr, 
            metadata_expr
        ).unwrap();

        // Use the simple query method which should auto-track weights
        let address_pattern = expr!(space, "[2] address [2] $ $");
        
        // Query multiple times and verify weight tracking
        let mut query_results = Vec::new();
        
        for i in 1..=3 {
            println!("Query iteration {}", i);
            let mut match_index = 0;
            space.query(address_pattern, |expr| {
                let path = unsafe { expr.span().as_ref().unwrap() };
                let expr_str = sexpr!(space, expr);
                if let Some(weight) = space.get_expression_weight(path) {
                    println!("  Match {}: {} (weight: {})", match_index, expr_str, weight.local_count);
                    
                    // Validate match structure - we see we only get 2 address matches
                    match match_index {
                        0 => { assert_eq!(expr_str, "((address (state NY)))") }
                        1 => { assert_eq!(expr_str, "((address (postal_code 10021-3100)))") }
                        _ => { println!("Additional address match: {}", expr_str) }
                    }
                    
                    query_results.push((i, path.to_vec(), weight.local_count, expr_str));
                    match_index += 1;
                }
            });
        }
        
        // Verify that weights increase with each query
        assert!(!query_results.is_empty(), "Should find some address expressions");
        
        // Group by expression path and check weight progression
        let mut weight_progressions: std::collections::HashMap<Vec<u8>, Vec<(u64, String)>> = 
            std::collections::HashMap::new();
            
        for (iteration, path, weight, expr_str) in query_results {
            weight_progressions.entry(path).or_insert_with(Vec::new).push((weight, expr_str));
        }
        
        for (path, weight_data) in weight_progressions {
            println!("Weight progression for expression '{}':", weight_data[0].1);
            for i in 1..weight_data.len() {
                let (prev_weight, _) = &weight_data[i-1];
                let (curr_weight, expr_str) = &weight_data[i];
                assert!(curr_weight > prev_weight, 
                        "Weight should increase with each query: {} -> {} for '{}'", 
                        prev_weight, curr_weight, expr_str);
                println!("  ✓ Weight increased from {} to {} for '{}'", 
                         prev_weight, curr_weight, expr_str);
            }
        }
    }
}
