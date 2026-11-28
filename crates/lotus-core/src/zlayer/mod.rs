//! Z-layer management system for AI integration.
extern crate alloc;

pub mod manifest;
pub mod chaining;

pub use manifest::Manifest;
pub use chaining::MerkleChain;

use alloc::vec::Vec;
use crate::model::coord::Coord;

/// Z-layer identifier with turn and index for addressing
#[derive(Debug, Clone, PartialEq)]
pub struct LayerAddress {
    pub turn: u32,
    pub index: u32,
}

impl LayerAddress {
    pub fn new(turn: u32, index: u32) -> Self {
        Self { turn, index }
    }
}

/// Status of a Z-layer in the manifest
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerStatus {
    /// Layer is pending review (not yet accepted/rejected)
    Pending,
    /// Layer was accepted and applied to workbook  
    Accepted,
    /// Layer was rejected but kept for reference
    Rejected,
    /// Layer was kept as note for later reference
    Kept,
}

/// Z-layer with metadata about its status and origin
#[derive(Debug, Clone)]
pub struct ZLayer {
    pub address: LayerAddress,
    pub content: Vec<(Coord, Option<crate::model::cell::Cell>)>,
    pub status: LayerStatus,
    /// Optional salt for Merkle chaining (128-bit minimum)
    pub salt: [u8; 16],
}

impl ZLayer {
    pub fn new(address: LayerAddress, content: Vec<(Coord, Option<crate::model::cell::Cell>)>) -> Self {
        let mut salt = [0u8; 16];
        // In a real implementation this would be properly randomized
        for i in 0..16 {
            salt[i] = (address.turn as u8).wrapping_add(address.index as u8).wrapping_add(i as u8);
        }
        
        Self { 
            address, 
            content,
            status: LayerStatus::Pending,
            salt
        }
    }
}