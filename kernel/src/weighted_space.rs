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
    pub fn query_multi_weighted<F>(&self, patterns: &[Expr], mut effect: F) -> usize 
    where 
        F: FnMut(Result<&[ExprEnv], (BTreeMap<(u8, u8), ExprEnv>, u8, u8, &[(u8, u8)])>, Expr, NodeWeight) -> bool,
    {
        let weights_ref = self.weights.clone();
        
        self.base_space.query_multi(patterns, |result, expr| {
            // Look up weight for this expression
            let weight = {
                let weights_guard = weights_ref.lock().unwrap();
                let path = unsafe { expr.span().as_ref().unwrap() };
                weights_guard.get_weight_copy(path).unwrap_or_default()
            };
            
            // Call the extended callback with weight information
            effect(result, expr, weight)
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use mork_bytestring::Tag;

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
}
