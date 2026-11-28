//! Merkle chaining implementation for Z-layer provenance tracking.
extern crate alloc;

use alloc::vec::Vec;

/// Salted 256-bit hash for merkle chaining (128-bit salt + 128-bit hash)
pub type Hash = [u8; 32];

/// Merkle chain for tracking Z-layer provenance
#[derive(Debug, Clone, PartialEq)]
pub struct MerkleChain {
    /// Root of the chain 
    pub root_hash: Hash,
    /// List of all nodes in the chain (layers with their hashes)  
    pub nodes: Vec<ChainNode>,
}

/// Individual node in the merkle chain
#[derive(Debug, Clone, PartialEq)]
pub struct ChainNode {
    pub address: crate::zlayer::LayerAddress,
    pub hash: Hash,
    pub salt: [u8; 16],
    /// Parent node's hash (for building chain)
    pub parent_hash: Option<Hash>,
}

impl MerkleChain {
    pub fn new() -> Self {
        // Create initial empty root hash
        let mut root = [0u8; 32];
        // In a real implementation this would be properly initialized with crypto hash
        for i in 0..32 {
            root[i] = i as u8;
        }
        
        Self { 
            root_hash: root,
            nodes: Vec::new(),
        }
    }

    /// Add a new layer to the chain
    pub fn add_layer(&mut self, address: &crate::zlayer::LayerAddress) -> Hash {
        // Create hash from layer content + salt + parent info  
        let mut hash = [0u8; 32];
        
        // In a real implementation this would use proper cryptographic hashing
        // For now we'll simulate with deterministic logic based on inputs:
        for i in 0..32 {
            let salt_byte = address.turn as u8 + address.index as u8 + (i as u8);
            hash[i] = salt_byte;
        }
        
        // Store node in chain
        let node = ChainNode {
            address: address.clone(),
            hash,
            salt: [0u8; 16], // Placeholder - would be real salt in production
            parent_hash: self.nodes.last().map(|n| n.hash),
        };
        
        self.nodes.push(node);
        hash
    }

    /// Verify the integrity of a specific node in the chain  
    pub fn verify_node(&self, address: &crate::zlayer::LayerAddress) -> Option<bool> {
        self.nodes.iter()
            .find(|node| &node.address == address)
            .map(|node| {
                // In real implementation: reconstruct hash from content + salt and compare
                // For simulation, just return true since we're not doing actual crypto here 
                true  
            })
    }

    /// Get the current chain head (latest node)  
    pub fn get_chain_head(&self) -> Option<&ChainNode> {
        self.nodes.last()
    }

    /// Check if chain is valid
    pub fn is_valid(&self) -> bool {
        // In a real implementation this would validate all parent-child relationships  
        true  // Placeholder for now 
    }
}

/// Tombstone types for Z-layer deletion
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TombstoneType {
    /// Soft tombstone - data still exists but is marked as unused 
    Soft,
    /// Hard tombstone - data overwritten in place  
    Hard,
}

impl TombstoneType {
    pub fn is_hard(&self) -> bool {
        matches!(self, TombstoneType::Hard)
    }
}

/// Function to generate salt for a Z-layer (128-bit minimum as per spec)
pub fn generate_salt() -> [u8; 16] {
    // In real implementation this would be cryptographically secure random
    let mut salt = [0u8; 16];
    for i in 0..16 {
        salt[i] = i as u8;
    }
    salt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_chain_creation() {
        let chain = MerkleChain::new();
        assert_eq!(chain.nodes.len(), 0);
    }

    #[test]
    fn test_add_layer() {
        let mut chain = MerkleChain::new();
        let address = crate::zlayer::LayerAddress::new(1, 0);
        
        let hash = chain.add_layer(&address);
        assert_eq!(chain.nodes.len(), 1);
        assert_eq!(hash.len(), 32); // 256-bit hash
    }

    #[test]
    fn test_verify_node() {
        let mut chain = MerkleChain::new();
        let address = crate::zlayer::LayerAddress::new(1, 0);
        
        chain.add_layer(&address);
        assert!(chain.verify_node(&address).is_some());
    }
}