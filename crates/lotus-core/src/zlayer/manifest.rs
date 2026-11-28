//! Manifest system for tracking Z-layer status and provenance.
extern crate alloc;

use alloc::vec::Vec;
use crate::zlayer::{LayerAddress, LayerStatus};

/// Manifest tracking the status of all Z-layers in a workbook
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    /// List of all layers with their current status 
    pub layers: Vec<ManifestEntry>,
    /// Current turn indicator (who's allowed to make changes)
    pub current_turn: u32,
}

/// Entry in the manifest tracking a single Z-layer's status
#[derive(Debug, Clone, PartialEq)]
pub struct ManifestEntry {
    pub address: LayerAddress,
    pub status: LayerStatus,
    /// Timestamp or version information for ordering
    pub timestamp: u64,
}

impl Manifest {
    pub fn new() -> Self {
        Self { 
            layers: Vec::new(),
            current_turn: 0, // Human starts with turn
        }
    }

    /// Add a new layer to the manifest  
    pub fn add_layer(&mut self, address: LayerAddress, timestamp: u64) {
        let entry = ManifestEntry {
            address,
            status: LayerStatus::Pending,
            timestamp,
        };
        self.layers.push(entry);
    }

    /// Update the status of a specific layer
    pub fn update_status(&mut self, address: &LayerAddress, new_status: LayerStatus) -> bool {
        for entry in &mut self.layers {
            if entry.address == *address {
                entry.status = new_status;
                return true;
            }
        }
        false
    }

    /// Get the status of a specific layer  
    pub fn get_status(&self, address: &LayerAddress) -> Option<LayerStatus> {
        self.layers.iter()
            .find(|entry| entry.address == *address)
            .map(|entry| entry.status)
    }

    /// Mark all pending layers from current turn as accepted
    pub fn accept_all_pending_from_turn(&mut self, turn: u32) {
        for entry in &mut self.layers {
            if entry.address.turn == turn && entry.status == LayerStatus::Pending {
                entry.status = LayerStatus::Accepted;
            }
        }
    }

    /// Mark all pending layers from current turn as rejected
    pub fn reject_all_pending_from_turn(&mut self, turn: u32) {
        for entry in &mut self.layers {
            if entry.address.turn == turn && entry.status == LayerStatus::Pending {
                entry.status = LayerStatus::Rejected;
            }
        }
    }

    /// Get all pending layers
    pub fn get_pending_layers(&self) -> Vec<&ManifestEntry> {
        self.layers.iter()
            .filter(|entry| entry.status == LayerStatus::Pending)
            .collect()
    }

    /// Check if there are any pending layers from current turn  
    pub fn has_pending_from_turn(&self, turn: u32) -> bool {
        self.layers.iter()
            .any(|entry| entry.address.turn == turn && entry.status == LayerStatus::Pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_creation() {
        let manifest = Manifest::new();
        assert_eq!(manifest.current_turn, 0);
        assert_eq!(manifest.layers.len(), 0);
    }

    #[test]
    fn test_add_layer() {
        let mut manifest = Manifest::new();
        let address = LayerAddress::new(1, 0);
        manifest.add_layer(address.clone(), 100);
        
        assert_eq!(manifest.layers.len(), 1);
        assert_eq!(manifest.layers[0].address, address);
        assert_eq!(manifest.layers[0].status, LayerStatus::Pending);
    }

    #[test]
    fn test_update_status() {
        let mut manifest = Manifest::new();
        let address = LayerAddress::new(1, 0);
        manifest.add_layer(address.clone(), 100);
        
        assert!(manifest.update_status(&address, LayerStatus::Accepted));
        assert_eq!(manifest.get_status(&address), Some(LayerStatus::Accepted));
    }

    #[test]
    fn test_accept_all_pending() {
        let mut manifest = Manifest::new();
        manifest.add_layer(LayerAddress::new(1, 0), 100);
        manifest.add_layer(LayerAddress::new(1, 1), 101);
        manifest.add_layer(LayerAddress::new(2, 0), 102);
        
        manifest.accept_all_pending_from_turn(1);
        
        assert_eq!(manifest.get_status(&LayerAddress::new(1, 0)), Some(LayerStatus::Accepted));
        assert_eq!(manifest.get_status(&LayerAddress::new(1, 1)), Some(LayerStatus::Accepted));
        assert_eq!(manifest.get_status(&LayerAddress::new(2, 0)), Some(LayerStatus::Pending));
    }
}