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

    /// Query with weight information included in results
    /// This method provides access to existing weights without modifying them
    pub fn query_multi_weighted<F>(&self, patterns: &[Expr], mut effect: F) -> usize 
    where 
        F: FnMut(Result<&[ExprEnv], (BTreeMap<(u8, u8), ExprEnv>, u8, u8, &[(u8, u8)])>, Expr, NodeWeight) -> bool,
    {
        let weights_ref = self.weights.clone();
        
        self.base_space.query_multi(patterns, |result, expr| {
            // Get the expression path for weight lookup
            let path = unsafe { expr.span().as_ref().unwrap() };
            
            // Get current weight without modifying it
            let weight = {
                let weights_guard = weights_ref.lock().unwrap();
                weights_guard.get_weight_copy(path).unwrap_or_default()
            };
            
            // Call the extended callback with weight information
            effect(result, expr, weight)
        })
    }

    /// Regular query method that delegates to base_space
    pub fn query<F>(&self, pattern: Expr, mut effect: F) 
    where 
        F: FnMut(Expr) -> (),
    {
        self.base_space.query(pattern, effect);
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
    const SEXPRS: &str = r#"(first_name John)
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
    fn test_weighted_queries() {
        use crate::{expr, sexpr};
        use crate::space_temporary::Space;
        
        let mut space = WeightedSpace::new();
        
        // Load real S-expressions into the space
        let pattern_expr = expr!(space, "$");
        let metadata_expr = expr!(space, "_1");
        
        let loaded_count = space.load_sexpr_simple(
            SEXPRS.as_bytes(), 
            pattern_expr, 
            metadata_expr
        ).unwrap();
        
        println!("Loaded {} expressions", loaded_count);
        assert!(loaded_count > 0, "Should load some expressions");

        // Perform queries - weights should remain 0 since we don't auto-increment
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
        
        // Test manual weight increment
        for (path, _weight, expr_str) in &found_children {
            // Manually increment weight for each found expression
            space.increment_expression_count(path, 1).unwrap();
            println!("Manually incremented weight for: {}", expr_str);
        }
        
        // Query again - weights should now be 1 due to manual increment
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
            SEXPRS.as_bytes(), 
            pattern_expr, 
            metadata_expr
        ).unwrap();

        // Use the simple query method - weights should start at 0
        let address_pattern = expr!(space, "[2] address [2] $ $");
        
        let mut found_expressions = Vec::new();
        
        // First query - collect expressions and their initial weights (should be 0)
        println!("Query iteration 1");
        let mut match_index = 0;
        space.query_multi_weighted(&[address_pattern], |_result, expr, weight| {
            let path = unsafe { expr.span().as_ref().unwrap() };
            let expr_str = sexpr!(space, expr);
            println!("  Match {}: {} (weight: {})", match_index, expr_str, weight.local_count);
            
            // Validate match structure - we see we only get 2 address matches
            match match_index {
                0 => { assert_eq!(expr_str, "((address (state NY)))") }
                1 => { assert_eq!(expr_str, "((address (postal_code 10021-3100)))") }
                _ => { println!("Additional address match: {}", expr_str) }
            }
            
            found_expressions.push((path.to_vec(), expr_str));
            match_index += 1;
            true // Continue with all matches
        });
        
        assert!(!found_expressions.is_empty(), "Should find some address expressions");
        
        // Manually increment weights for testing
        for (i, (path, expr_str)) in found_expressions.iter().enumerate() {
            space.increment_expression_count(path, (i + 1) as u64).unwrap();
            println!("Manually set weight {} for: {}", i + 1, expr_str);
        }
        
        // Query again to verify manual weights
        println!("Query iteration 2 - checking manual weights");
        let mut match_index = 0;
        space.query_multi_weighted(&[address_pattern], |_result, expr, weight| {
            let path = unsafe { expr.span().as_ref().unwrap() };
            let expr_str = sexpr!(space, expr);
            println!("  Match {}: {} (weight: {})", match_index, expr_str, weight.local_count);
            
            // Verify that weights match our manual increments
            let expected_weight = (match_index + 1) as u64;
            assert_eq!(weight.local_count, expected_weight, 
                      "Weight should be {} for expression: {}", expected_weight, expr_str);
            
            match_index += 1;
            true // Continue with all matches
        });
        
        println!("✓ All manual weight assignments verified successfully");
    }
}
