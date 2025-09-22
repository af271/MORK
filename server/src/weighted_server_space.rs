use std::time::Duration;
use std::sync::Mutex;

use hyper::StatusCode;

use pathmap::{PathMap, zipper::ZipperHeadOwned};
use pathmap::zipper_tracking::Conflict;
use pathmap::zipper::*;
use pathmap::weighted_trie::{WeightedTriemap, NodeWeight};

use mork::{PermissionArb, PathPermissionErr, Space, SpaceReaderZipper, SpaceWriterZipper};
use mork::weighted_space::{WeightedSpace, WeightedQueryResult};

use crate::status_map::*;
use crate::commands::*;

/// The time to wait before rejecting a request with a conflicted path
const SETTLE_TIME: Duration = Duration::from_millis(5);

/// A server space that supports weighted expression tracking
pub struct WeightedServerSpace {
    /// The weighted space that tracks expression frequencies
    pub weighted_space: WeightedSpace,
    /// ZipperHead for accessing status and permissions associated with paths
    pub(crate) status_map: StatusMap,
    /// Guard to ensure high-level operations can be atomic
    permission_guard: Mutex<()>,
}

/// PermissionHead object for [WeightedServerSpace]
pub struct WeightedServerPermissionHead<'space>(&'space WeightedServerSpace);

impl WeightedServerSpace {
    /// Make a new `WeightedServerSpace`
    pub fn new() -> Self {
        Self {
            weighted_space: WeightedSpace::new_with_k(100), // Track top 100 expressions
            status_map: StatusMap::new(),
            permission_guard: Mutex::new(()),
        }
    }

    /// Add an expression with a specific weight/count
    pub fn add_expression_with_count(&mut self, expr_bytes: &[u8], count: u64) -> Result<(), String> {
        self.weighted_space.add_expression_weighted(expr_bytes, count)
    }

    /// Query expressions and return results with weights
    pub fn query_weighted(&self, patterns: &[mork_bytestring::Expr]) -> Vec<WeightedQueryResult> {
        self.weighted_space.query_weighted_collect(patterns)
    }

    /// Get the top-k most frequent expressions
    pub fn get_top_expressions(&self, k: usize) -> Vec<WeightedQueryResult> {
        self.weighted_space.get_top_k_expressions(k)
    }

    /// Increment the count for an existing expression
    pub fn increment_expression(&self, expr_bytes: &[u8], count: u64) -> Result<(), String> {
        self.weighted_space.increment_expression_count(expr_bytes, count)
    }

    /// Get weight information for a specific expression
    pub fn get_expression_weight(&self, expr_bytes: &[u8]) -> Option<NodeWeight> {
        self.weighted_space.get_expression_weight(expr_bytes)
    }

    /// Query for top-k heaviest expressions matching patterns
    pub fn query_top_k(&self, patterns: &[mork_bytestring::Expr], k: usize) -> Vec<WeightedQueryResult> {
        self.weighted_space.query_top_k_weighted(patterns, k)
    }
}

// Delegate basic space operations to the underlying weighted space
impl std::ops::Deref for WeightedServerSpace {
    type Target = WeightedSpace;
    
    fn deref(&self) -> &Self::Target {
        &self.weighted_space
    }
}

impl std::ops::DerefMut for WeightedServerSpace {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.weighted_space
    }
}

impl PermissionArb for WeightedServerPermissionHead<'_> {
    type WriterT = SpaceWriterZipper<'static>;
    type ReaderT = SpaceReaderZipper<'static>;

    fn grant_rd_permissions(
        &mut self,
        exprs: &[impl mork_bytestring::ExprTrait],
    ) -> Result<Vec<Self::ReaderT>, PathPermissionErr> {
        // Delegate to the underlying space
        let mut base_perm = self.0.weighted_space.new_permission_arb();
        base_perm.grant_rd_permissions(exprs)
    }

    fn grant_wr_permissions(
        &mut self,
        exprs: &[impl mork_bytestring::ExprTrait],
    ) -> Result<Vec<Self::WriterT>, PathPermissionErr> {
        // Delegate to the underlying space
        let mut base_perm = self.0.weighted_space.new_permission_arb();
        base_perm.grant_wr_permissions(exprs)
    }
}

impl Space for WeightedServerSpace {
    type PermissionT = WeightedServerPermissionHead<'_>;
    type WriterT = SpaceWriterZipper<'static>;
    type ReaderT = SpaceReaderZipper<'static>;

    fn new_permission_arb(&self) -> Self::PermissionT {
        WeightedServerPermissionHead(self)
    }

    fn grant_rd_permissions(
        &self,
        permissions: &mut Self::PermissionT,
        exprs: &[impl mork_bytestring::ExprTrait],
    ) -> Result<Vec<Self::ReaderT>, PathPermissionErr> {
        permissions.grant_rd_permissions(exprs)
    }

    fn grant_wr_permissions(
        &self,
        permissions: &mut Self::PermissionT,
        exprs: &[impl mork_bytestring::ExprTrait],
    ) -> Result<Vec<Self::WriterT>, PathPermissionErr> {
        permissions.grant_wr_permissions(exprs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mork_bytestring::Tag;

    #[test]
    fn test_weighted_server_space_creation() {
        let space = WeightedServerSpace::new();
        // Just verify it doesn't panic
        assert!(space.get_top_expressions(10).is_empty());
    }

    #[test]
    fn test_expression_tracking() {
        let mut space = WeightedServerSpace::new();
        
        // Create a simple expression
        let expr_bytes = vec![
            Tag::Arity(1).encode_as_byte().unwrap(),
            Tag::SymbolSize(4).encode_as_byte().unwrap(),
            b't', b'e', b's', b't',
        ];

        // Add it multiple times
        space.add_expression_with_count(&expr_bytes, 5).unwrap();
        space.increment_expression(&expr_bytes, 3).unwrap();
        
        // Check the total count
        let weight = space.get_expression_weight(&expr_bytes).unwrap();
        assert_eq!(weight.local_count, 8);
    }
}
