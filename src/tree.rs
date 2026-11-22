//! Merkle tree with sorted-pair hashing.
//!
//! The construction matches `merkletreejs` with `{ sortPairs: true }` and
//! verifies with OpenZeppelin's `MerkleProof` library: every pair of nodes is
//! sorted before hashing, and an unpaired node at the end of a layer is
//! promoted to the next layer unchanged.

use crate::{keccak256, Hash};

/// Hashes two nodes in sorted order (commutative).
pub fn hash_pair(a: &Hash, b: &Hash) -> Hash {
    let (first, second) = if a <= b { (a, b) } else { (b, a) };
    let mut buffer = [0u8; 64];
    buffer[..32].copy_from_slice(first);
    buffer[32..].copy_from_slice(second);
    keccak256(&buffer)
}

/// A fully materialised Merkle tree. Proof lookups are `O(log n)`.
pub struct MerkleTree {
    /// `layers[0]` holds the leaves, the last layer holds only the root.
    layers: Vec<Vec<Hash>>,
}

impl MerkleTree {
    /// Builds a tree from leaf hashes, preserving their order.
    ///
    /// # Panics
    ///
    /// Panics if `leaves` is empty.
    pub fn new(leaves: Vec<Hash>) -> Self {
        assert!(!leaves.is_empty(), "a Merkle tree needs at least one leaf");

        let mut layers = vec![leaves];
        while layers[layers.len() - 1].len() > 1 {
            let next = layers[layers.len() - 1]
                .chunks(2)
                .map(|pair| match pair {
                    [left, right] => hash_pair(left, right),
                    [single] => *single,
                    _ => unreachable!(),
                })
                .collect();
            layers.push(next);
        }

        Self { layers }
    }

    pub fn root(&self) -> Hash {
        self.layers[self.layers.len() - 1][0]
    }

    pub fn leaves(&self) -> &[Hash] {
        &self.layers[0]
    }

    /// Returns the proof (sibling hashes from leaf to root) for a leaf index.
    pub fn proof(&self, mut index: usize) -> Vec<Hash> {
        let mut proof = Vec::with_capacity(self.layers.len());
        for layer in &self.layers[..self.layers.len() - 1] {
            if let Some(sibling) = layer.get(index ^ 1) {
                proof.push(*sibling);
            }
            index /= 2;
        }
        proof
    }
}

/// Verifies a proof the same way OpenZeppelin's `MerkleProof.verify` does.
pub fn verify(proof: &[Hash], root: &Hash, leaf: &Hash) -> bool {
    let computed = proof.iter().fold(*leaf, |node, sibling| hash_pair(&node, sibling));
    computed == *root
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(count: usize) -> Vec<Hash> {
        (0..count).map(|i| keccak256(&(i as u64).to_be_bytes())).collect()
    }

    #[test]
    fn single_leaf_is_its_own_root() {
        let leaves = leaves(1);
        let tree = MerkleTree::new(leaves.clone());
        assert_eq!(tree.root(), leaves[0]);
        assert!(tree.proof(0).is_empty());
        assert!(verify(&[], &tree.root(), &leaves[0]));
    }

    #[test]
    fn every_proof_verifies_for_any_size() {
        for count in 1..=65 {
            let leaves = leaves(count);
            let tree = MerkleTree::new(leaves.clone());
            let root = tree.root();
            for (index, leaf) in leaves.iter().enumerate() {
                assert!(verify(&tree.proof(index), &root, leaf), "size {count}, index {index}");
            }
        }
    }

    #[test]
    fn rejects_foreign_leaf_and_tampered_proof() {
        let leaves = leaves(8);
        let tree = MerkleTree::new(leaves.clone());
        let root = tree.root();

        let outsider = keccak256(b"not in the tree");
        assert!(!verify(&tree.proof(3), &root, &outsider));

        let mut proof = tree.proof(3);
        proof[0][0] ^= 1;
        assert!(!verify(&proof, &root, &leaves[3]));
    }
}
