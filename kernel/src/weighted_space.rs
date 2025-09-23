use std::sync::{Arc, Mutex};
use std::collections::BTreeMap;
use std::ops::Deref;

use crate::space::DefaultSpace;
use crate::{expr, sexpr};
use crate::space_temporary::Space;
use crate::space::serialize_sexpr_into;
use pathmap::weighted_trie::{WeightedTriemap, NodeWeight};
use mork_bytestring::{Expr, ExprEnv};


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
            // Convert expression to string representation and remove extra wrapper parentheses
            let mut expr_buffer = Vec::new();
            let weight = if let Ok(()) = serialize_sexpr_into(expr.ptr, &mut expr_buffer, self.base_space.symbol_table()) {
                // Remove first and last byte (the extra parentheses) to match add_expression_weighted format
                if expr_buffer.len() >= 2 && expr_buffer[0] == b'(' && expr_buffer[expr_buffer.len()-1] == b')' {
                    let trimmed_bytes = &expr_buffer[1..expr_buffer.len()-1];
                    let weights_guard = weights_ref.lock().unwrap();
                    weights_guard.get_weight_copy(trimmed_bytes).unwrap_or_default()
                } else {
                    NodeWeight::default()
                }
            } else {
                NodeWeight::default()
            };
            
            // Call the extended callback with weight information
            effect(result, expr, weight)
        })
    }

    /// Add an expression with weight tracking
    /// This increments the local_count by 1 for the expression
    pub fn add_expression_weighted(&mut self, expr_str: &str) -> Result<(), String> {       
        let pattern_expr = expr!(self.base_space, "$");
        let metadata_expr = expr!(self.base_space, "_1");

        // Load the expression
        let loaded_count = self.base_space.load_sexpr_simple(expr_str.as_bytes(), pattern_expr, metadata_expr)
            .map_err(|e| format!("Failed to load expression: {}", e))?;
        
        // Increment weight using the expression string as path
        {
            let mut weights = self.weights.lock().unwrap();
            weights.increment_count(expr_str.as_bytes());
        }
        
        Ok(())
    }

    /// Get weight information for a specific expression
    pub fn get_expression_weight(&self, expr: &[u8]) -> Option<NodeWeight> {
        let weights = self.weights.lock().unwrap();
        weights.get_weight_copy(expr)
    }

    /// Query expressions matching a pattern and return the top-k by weight
    /// This combines pattern matching with weight-based ranking
    pub fn query_top_k(&self, pattern: Expr, k: usize) -> Vec<(Expr, NodeWeight)> {
        let mut weighted_results = Vec::new();
        
        // Collect all matching expressions with their weights
        self.query_multi_weighted(&[pattern], |_result, expr, weight| {
            if weight.local_count > 0 {
                weighted_results.push((expr, weight));
            }
            true // Continue collecting all results
        });
        
        // Sort by weight (highest first) and take top k
        weighted_results.sort_by(|a, b| b.1.local_count.partial_cmp(&a.1.local_count).unwrap_or(std::cmp::Ordering::Equal));
        weighted_results.truncate(k);
        
        weighted_results
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
    fn test_add_expression_weighted() {
        let mut space = WeightedSpace::new();
        
        // Test adding a simple expression
        let expr_str = "(+ a b)";
        space.add_expression_weighted(expr_str).unwrap();
        
        // Test that it was added successfully
    }
    
    #[test]
    fn test_weighted_queries() {
        let mut space = WeightedSpace::new();
        
        // Add expressions individually using add_expression_weighted
        // Each expression gets added a different number of times to create different weights
        
        // Add some expressions multiple times
        let expressions = [
            "(first_name John)",       // Add 3 times -> weight = 3
            "(last_name Smith)",       // Add 1 time -> weight = 1  
            "(age 27)",               // Add 5 times -> weight = 5
            "(children (0 Catherine))", // Add 2 times -> weight = 2
            "(children (1 Thomas))",   // Add 4 times -> weight = 4
            "(children (2 Trevor))",   // Add 1 time -> weight = 1
        ];
        
        let add_counts = [3, 1, 5, 2, 4, 1]; // How many times to add each expression
        
        // Add each expression the specified number of times
        for (expr_str, &count) in expressions.iter().zip(add_counts.iter()) {
            for i in 0..count {
                space.add_expression_weighted(expr_str).unwrap();
            }
        }
        
        // Test query for children specifically to verify weights
        let children_pattern = expr!(space, "[2] children [2] $ $");
        
        let mut found_children = Vec::new();
        let mut match_index = 0;
        let query_count = space.query_multi_weighted(&[children_pattern], |_result, expr, weight| {
            let path = unsafe { expr.span().as_ref().unwrap() };
            let expr_str = sexpr!(space, expr);
            
            // Verify the expected weights for children expressions
            match expr_str.as_str() {
                "((children (0 Catherine)))" => {
                    assert_eq!(weight.local_count, 2, "Catherine should have weight 2");
                }
                "((children (1 Thomas)))" => {
                    assert_eq!(weight.local_count, 4, "Thomas should have weight 4");
                }
                "((children (2 Trevor)))" => {
                    assert_eq!(weight.local_count, 1, "Trevor should have weight 1");
                }
                _ => {
                    // Allow other unexpected expressions
                }
            }
            
            found_children.push((path.to_vec(), weight, expr_str.clone()));
            match_index += 1;
            true // Continue collecting all results
        });

        assert_eq!(query_count, 3, "Should find exactly 3 children");
    }

    #[test]
    fn test_query_top_k() {
        let mut space = WeightedSpace::new();
        
        // Add different children expressions with varying frequencies
        let expressions = [
            "(children (name Alice))",     // Will be added 1 time  -> weight = 1
            "(children (name Bob))",       // Will be added 3 times -> weight = 3
            "(age 27)",                    // Will be added 13 times -> weight = 13  
            "(children (name Charlie))",   // Will be added 2 times -> weight = 2
            "(children (name David))",     // Will be added 5 times -> weight = 5
        ];
        
        let add_counts = [1, 3, 13, 2, 5];
        
        // Add each expression the specified number of times
        for (expr_str, &count) in expressions.iter().zip(add_counts.iter()) {
            for _ in 0..count {
                space.add_expression_weighted(expr_str).unwrap();
            }
        }
        
        // Test query_top_k with a pattern that matches all children
        let children_pattern = expr!(space, "[2] children [2] $ $");
        let top_2_children = space.query_top_k(children_pattern, 2);
        
        // Should return top 2 by weight: David (5) and Bob (3)
        assert_eq!(top_2_children.len(), 2, "Should return exactly 2 results");
        
        // Verify the highest weight is David with weight 5
        assert_eq!(top_2_children[0].1.local_count, 5, "Highest should be David with weight 5");
        
        // Verify the second highest weight is Bob with weight 3
        assert_eq!(top_2_children[1].1.local_count, 3, "Second should be Bob with weight 3");
        
        // Test with k larger than available results
        let top_10_children = space.query_top_k(children_pattern, 10);
        assert_eq!(top_10_children.len(), 4, "Should return all 4 available results when k > available");
        
        // Verify the ordering: David(5), Bob(3), Charlie(2), Alice(1)
        let expected_weights = [5, 3, 2, 1];
        for (i, expected_weight) in expected_weights.iter().enumerate() {
            assert_eq!(top_10_children[i].1.local_count, *expected_weight, 
                      "Result {} should have weight {}", i, expected_weight);
        }
    }
}
