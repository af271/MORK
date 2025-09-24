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

    /// Extracts all subtrees from an s-expression string
    pub fn extract_subtrees(input: &str) -> Vec<String> {
        let mut subtrees = Vec::new();
        let mut stack = Vec::new();
        let mut in_string = false;
        let mut escape_next = false;
        
        let chars: Vec<char> = input.chars().collect();
        
        for (i, &c) in chars.iter().enumerate() {
            if escape_next {
                escape_next = false;
                continue;
            }
            
            match c {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                '(' if !in_string => {
                    stack.push(i);
                }
                ')' if !in_string => {
                    if let Some(start) = stack.pop() {
                        let subtree = chars[start..=i].iter().collect::<String>();
                        subtrees.push(subtree);
                    }
                }
                _ => {}
            }
        }
        
        // Remove duplicates and sort
        subtrees.sort();
        subtrees.dedup();
        subtrees
    }

    /// Query with weight information included in results
    /// This method provides access to existing weights without modifying them
    pub fn query_multi<F>(&self, patterns: &[Expr], mut effect: F) -> usize 
    where 
        F: FnMut(Result<&[ExprEnv], (BTreeMap<(u8, u8), ExprEnv>, u8, u8, &[(u8, u8)])>, Expr, NodeWeight) -> bool,
    {
        let weights_ref = self.weights.clone();
        
        self.base_space.query_multi(patterns, |result, expr| {
            // Convert expression to string representation and remove extra wrapper parentheses
            let mut expr_buffer = Vec::new();
            let weight = if let Ok(()) = serialize_sexpr_into(expr.ptr, &mut expr_buffer, self.base_space.symbol_table()) {
                // Remove first and last byte (the extra parentheses) to match add_expression format
                if expr_buffer.len() >= 2 && expr_buffer[0] == b'(' && expr_buffer[expr_buffer.len()-1] == b')' {
                    let trimmed_bytes = &expr_buffer[1..expr_buffer.len()-1];
                    let weights_guard = weights_ref.lock().unwrap();
                    weights_guard.get_weight(trimmed_bytes).unwrap_or_default()
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
    /// This increments the local_count by 1 for the expression and all its subtrees
    pub fn add_expression(&mut self, expr_str: &str) -> Result<(), String> {       
        let pattern_expr = expr!(self.base_space, "$");
        let metadata_expr = expr!(self.base_space, "_1");

        // Load the expression
        let _loaded_count = self.base_space.load_sexpr_simple(expr_str.as_bytes(), pattern_expr, metadata_expr)
            .map_err(|e| format!("Failed to load expression: {}", e))?;
        
        // Extract all subtrees from the expression
        let subtrees = Self::extract_subtrees(expr_str);
        
        // Increment weight for the main expression and all subtrees (but avoid double counting)
        {
            let mut weights = self.weights.lock().unwrap();
            
            // Increment the main expression
            weights.increment_count(expr_str.as_bytes());
            
            // Increment subtrees that are different from the main expression
            for subtree in subtrees {
                if subtree != expr_str {
                    weights.increment_count(subtree.as_bytes());
                }
            }
        }
        
        Ok(())
    }

    /// Get weight information for a specific expression
    pub fn get_expression_weight(&self, expr: &[u8]) -> Option<NodeWeight> {
        let weights = self.weights.lock().unwrap();
        weights.get_weight(expr)
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
    fn test_add_expression() {
        let mut space = WeightedSpace::new();
        
        // Test adding a simple expression
        let expr_str = "(+ a b)";
        space.add_expression(expr_str).unwrap();
        
        // Test that it was added successfully
    }

    #[test]
    fn test_extract_subtrees() {
        let input = "(f (g a b) (h c))";
        let subtrees = WeightedSpace::extract_subtrees(input);
        
        // Should extract all complete s-expressions
        let expected = vec![
            "(f (g a b) (h c))".to_string(),
            "(g a b)".to_string(),
            "(h c)".to_string()
        ];
        
        assert_eq!(subtrees.len(), 3);
        for expected_subtree in expected {
            assert!(subtrees.contains(&expected_subtree), 
                   "Should contain subtree: {}", expected_subtree);
        }
    }

    #[test] 
    fn test_add_expression_with_subtrees() {
        let mut space = WeightedSpace::new();
        
        // Add a complex expression that should generate subtrees
        let expr_str = "(f (g a b) (h c))";
        space.add_expression(expr_str).unwrap();
        
        // Check that the main expression has weight 1
        let main_weight = space.get_expression_weight(expr_str.as_bytes());
        assert!(main_weight.is_some());
        assert_eq!(main_weight.unwrap().local_count, 1);
        
        // Check that subtrees also have weight 1
        let subtree1_weight = space.get_expression_weight(b"(g a b)");
        assert!(subtree1_weight.is_some());
        assert_eq!(subtree1_weight.unwrap().local_count, 1);
        
        let subtree2_weight = space.get_expression_weight(b"(h c)");
        assert!(subtree2_weight.is_some());
        assert_eq!(subtree2_weight.unwrap().local_count, 1);
        
        // Add the same expression again - all weights should increment
        space.add_expression(expr_str).unwrap();
        
        // Check that all weights are now 2
        let main_weight2 = space.get_expression_weight(expr_str.as_bytes());
        assert_eq!(main_weight2.unwrap().local_count, 2);
        
        let subtree1_weight2 = space.get_expression_weight(b"(g a b)");
        assert_eq!(subtree1_weight2.unwrap().local_count, 2);
        
        let subtree2_weight2 = space.get_expression_weight(b"(h c)");
        assert_eq!(subtree2_weight2.unwrap().local_count, 2);
    }
}
