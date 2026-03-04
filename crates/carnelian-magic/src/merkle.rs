//! Merkle Tree implementation for O(log n) memory integrity verification
//!
//! Provides efficient verification of memory entries using a Merkle tree structure.
//! Instead of verifying each memory individually (O(n)), we can verify the entire
//! tree with a single root hash and provide O(log n) proofs of inclusion.

use blake3::Hasher;
use serde::{Deserialize, Serialize};

/// Merkle tree for memory integrity verification
#[derive(Debug, Clone)]
pub struct MemoryMerkleTree {
    /// Root hash of the tree
    pub root_hash: [u8; 32],
    /// Leaf hashes (one per memory entry)
    leaves: Vec<[u8; 32]>,
    /// Internal node hashes (computed from leaves)
    #[allow(dead_code)]
    nodes: Vec<[u8; 32]>,
}

/// Merkle proof for a single leaf
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Index of the leaf in the tree
    pub leaf_index: usize,
    /// Sibling hashes along the path to root (None if no sibling exists)
    pub siblings: Vec<Option<[u8; 32]>>,
}

impl MemoryMerkleTree {
    /// Create a new Merkle tree from memory hashes
    ///
    /// # Arguments
    /// * `leaf_hashes` - Blake3 hashes of memory entries
    ///
    /// # Returns
    /// A new `MemoryMerkleTree` with computed root hash
    pub fn new(leaf_hashes: Vec<[u8; 32]>) -> Self {
        if leaf_hashes.is_empty() {
            return Self {
                root_hash: [0u8; 32],
                leaves: Vec::new(),
                nodes: Vec::new(),
            };
        }

        let leaves = leaf_hashes;
        let mut nodes = Vec::new();
        
        // Build tree bottom-up
        let mut current_level = leaves.clone();
        
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    // Hash pair of nodes
                    Self::hash_pair(&chunk[0], &chunk[1])
                } else {
                    // Odd node, promote to next level
                    chunk[0]
                };
                next_level.push(hash);
                nodes.push(hash);
            }
            
            current_level = next_level;
        }
        
        let root_hash = current_level[0];
        
        Self {
            root_hash,
            leaves,
            nodes,
        }
    }

    /// Hash a pair of nodes
    fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(left);
        hasher.update(right);
        *hasher.finalize().as_bytes()
    }

    /// Generate a Merkle proof for a leaf
    ///
    /// # Arguments
    /// * `leaf_index` - Index of the leaf to prove
    ///
    /// # Returns
    /// A `MerkleProof` containing sibling hashes along the path to root
    pub fn generate_proof(&self, leaf_index: usize) -> Option<MerkleProof> {
        if leaf_index >= self.leaves.len() {
            return None;
        }

        let mut siblings = Vec::new();
        let mut current_index = leaf_index;
        let mut current_level = self.leaves.clone();

        while current_level.len() > 1 {
            // Get sibling index
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            // Add sibling hash if it exists, otherwise add a marker for "no sibling"
            if sibling_index < current_level.len() {
                siblings.push(Some(current_level[sibling_index]));
            } else {
                siblings.push(None);
            }

            // Move to parent level
            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    Self::hash_pair(&chunk[0], &chunk[1])
                } else {
                    chunk[0]
                };
                next_level.push(hash);
            }

            current_index /= 2;
            current_level = next_level;
        }

        Some(MerkleProof {
            leaf_index,
            siblings,
        })
    }

    /// Verify a Merkle proof
    ///
    /// # Arguments
    /// * `leaf_hash` - Hash of the leaf to verify
    /// * `proof` - Merkle proof for the leaf
    ///
    /// # Returns
    /// `true` if the proof is valid, `false` otherwise
    pub fn verify_proof(&self, leaf_hash: &[u8; 32], proof: &MerkleProof) -> bool {
        let mut current_hash = *leaf_hash;
        let mut current_index = proof.leaf_index;

        for sibling_opt in &proof.siblings {
            if let Some(sibling) = sibling_opt {
                current_hash = if current_index % 2 == 0 {
                    // Current is left child
                    Self::hash_pair(&current_hash, sibling)
                } else {
                    // Current is right child
                    Self::hash_pair(sibling, &current_hash)
                };
            }
            // If no sibling, current_hash just propagates up unchanged
            current_index /= 2;
        }

        current_hash == self.root_hash
    }

    /// Update a leaf and recompute affected hashes
    ///
    /// # Arguments
    /// * `leaf_index` - Index of the leaf to update
    /// * `new_hash` - New hash for the leaf
    ///
    /// # Returns
    /// `true` if update succeeded, `false` if index out of bounds
    pub fn update_leaf(&mut self, leaf_index: usize, new_hash: [u8; 32]) -> bool {
        if leaf_index >= self.leaves.len() {
            return false;
        }

        // Update leaf
        self.leaves[leaf_index] = new_hash;

        // Recompute tree
        *self = Self::new(self.leaves.clone());
        
        true
    }

    /// Get the number of leaves in the tree
    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }

    /// Get the root hash
    pub fn root(&self) -> [u8; 32] {
        self.root_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_data(data: &[u8]) -> [u8; 32] {
        *blake3::hash(data).as_bytes()
    }

    #[test]
    fn test_empty_tree() {
        let tree = MemoryMerkleTree::new(Vec::new());
        assert_eq!(tree.leaf_count(), 0);
        assert_eq!(tree.root(), [0u8; 32]);
    }

    #[test]
    fn test_single_leaf() {
        let leaf = hash_data(b"memory1");
        let tree = MemoryMerkleTree::new(vec![leaf]);
        
        assert_eq!(tree.leaf_count(), 1);
        assert_eq!(tree.root(), leaf);
    }

    #[test]
    fn test_multiple_leaves() {
        let leaves = vec![
            hash_data(b"memory1"),
            hash_data(b"memory2"),
            hash_data(b"memory3"),
            hash_data(b"memory4"),
        ];
        
        let tree = MemoryMerkleTree::new(leaves.clone());
        assert_eq!(tree.leaf_count(), 4);
        assert_ne!(tree.root(), [0u8; 32]);
    }

    #[test]
    fn test_proof_generation_and_verification() {
        let leaves = vec![
            hash_data(b"memory1"),
            hash_data(b"memory2"),
            hash_data(b"memory3"),
            hash_data(b"memory4"),
        ];
        
        let tree = MemoryMerkleTree::new(leaves.clone());
        
        // Generate and verify proof for each leaf
        for (i, leaf) in leaves.iter().enumerate() {
            let proof = tree.generate_proof(i).unwrap();
            assert_eq!(proof.leaf_index, i);
            assert!(tree.verify_proof(leaf, &proof));
        }
    }

    #[test]
    fn test_proof_fails_on_wrong_leaf() {
        let leaves = vec![
            hash_data(b"memory1"),
            hash_data(b"memory2"),
            hash_data(b"memory3"),
        ];
        
        let tree = MemoryMerkleTree::new(leaves);
        
        let proof = tree.generate_proof(0).unwrap();
        let wrong_leaf = hash_data(b"wrong_memory");
        
        assert!(!tree.verify_proof(&wrong_leaf, &proof));
    }

    #[test]
    fn test_update_leaf() {
        let leaves = vec![
            hash_data(b"memory1"),
            hash_data(b"memory2"),
            hash_data(b"memory3"),
        ];
        
        let mut tree = MemoryMerkleTree::new(leaves.clone());
        let original_root = tree.root();
        
        // Update leaf
        let new_hash = hash_data(b"updated_memory1");
        assert!(tree.update_leaf(0, new_hash));
        
        // Root should change
        assert_ne!(tree.root(), original_root);
        
        // New proof should verify
        let proof = tree.generate_proof(0).unwrap();
        assert!(tree.verify_proof(&new_hash, &proof));
    }

    #[test]
    fn test_odd_number_of_leaves() {
        let leaves = vec![
            hash_data(b"memory1"),
            hash_data(b"memory2"),
            hash_data(b"memory3"),
            hash_data(b"memory4"),
            hash_data(b"memory5"),
        ];
        
        let tree = MemoryMerkleTree::new(leaves.clone());
        
        // All proofs should verify
        for (i, leaf) in leaves.iter().enumerate() {
            let proof = tree.generate_proof(i).unwrap();
            assert!(tree.verify_proof(leaf, &proof));
        }
    }
}
