use std::sync::{Arc, Mutex};

use pathmap::weighted_trie::{WeightedTriemap, NodeWeight};

/// Weight tracking system for ServerSpace operations
/// This tracks expression frequencies parallel to ServerSpace operations
pub struct ServerWeightedSpace {
    /// Weight tracking trie (parallel to ServerSpace operations)
    pub weights: Arc<Mutex<WeightedTriemap<NodeWeight>>>,
}

impl ServerWeightedSpace {
    /// Creates a new weighted space tracker
    pub fn new() -> Self {
        Self {
            weights: Arc::new(Mutex::new(WeightedTriemap::new())),
        }
    }

    /// Add weight tracking for an expression
    /// This should be called whenever an expression is added to ServerSpace
    pub fn track_expression(&mut self, expr_str: &str) -> Result<(), String> {
        let mut weights = self.weights.lock()
            .map_err(|e| format!("Failed to lock weights: {}", e))?;
        
        // Convert expression string to bytes for weight tracking
        let expr_bytes = expr_str.as_bytes();
        
        // Increment count for the expression path
        weights.increment_count(expr_bytes);
        
        Ok(())
    }
    
    /// Add expression with compatibility for old API
    /// This maintains compatibility with existing AddExpressionCmd
    pub fn add_expression(&mut self, expr_str: &str) -> Result<(), String> {
        self.track_expression(expr_str)
    }
    
    /// Get the weight/frequency of an expression
    pub fn get_expression_weight(&self, expr_str: &str) -> Option<f64> {
        let weights = self.weights.lock().ok()?;
        let expr_bytes = expr_str.as_bytes();
        weights.get_weight(expr_bytes).map(|w| w.local_count as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_weighted_space_creation() {
        let server_space = ServerSpace::new();
        let weighted_space = ServerWeightedSpace::new(server_space);
        
        // Should be able to add expressions
        // TODO: Add more comprehensive tests once add_expression is implemented
    }

    #[test] 
    fn test_weight_tracking() {
        let server_space = ServerSpace::new();
        let mut weighted_space = ServerWeightedSpace::new(server_space);
        
        // Add an expression
        weighted_space.add_expression("(test expression)").unwrap();
        
        // Check that weight was tracked
        let weight = weighted_space.get_expression_weight(b"(test expression)");
        assert!(weight.is_some());
        assert_eq!(weight.unwrap().local_count, 1);
    }
}
