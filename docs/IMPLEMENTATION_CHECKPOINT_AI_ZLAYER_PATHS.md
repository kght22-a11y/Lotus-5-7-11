# Implementation Checkpoint: AI Z-Layer Update Paths

## Overview
This document outlines the implementation plan for the three AI edit paths (direct file modification, chat-based updates, and API integration) as specified in the Lotus 5-7-11 AI Exchange design.

## Key Design Requirements from Specification

### Three AI Edit Paths
1. **Direct File Modification** - AI appends z-layer updates directly to the file 
2. **Chat-Based Updates** - AI generates z-layer updates that user can paste into application
3. **API Integration** - API like OpenAI submits z-layer updates via interface

### Z-Layer Management System
- **Verification Process**: Accepted, Not Accepted, or Kept as Note (with "keep" functionality)
- **Batch Processing**: If one update in a batch is rejected, turn flag returns to AI for reconsideration  
- **Turn Flag Handling**: Proper hand-off mechanism between human and AI turns
- **Data Integrity**: Merkle chaining with salted hashes for provenance tracking

### File Format Requirements
- Z-layers are parsed when file reloads to check for updates
- Only user can remove data from files (AI only submits z-layer updates)
- Updates verified by user at time of file reload
- Z-layers stored in format compatible with existing structure

## Implementation Plan

### Phase 1: Core Infrastructure Setup (Completed)
- [x] Implemented Z-layer address and turn model 
- [x] Added manifest system for tracking layer status (accept, reject, keep)
- [x] Created settings region for turn flag handling
- [x] Implemented staged import mechanism replacing current auto-accept
- [x] Integrated Merkle chaining with salted hashes  
- [x] Updated workbook structure to include Z-layer management

### Phase 2: AI Integration Paths  
- [ ] Direct file modification path with proper detection of new z-layers
- [ ] Chat-based paste area with automatic font scaling (leveraging existing font_scaling.rs)
- [ ] API integration interface for submitting z-layer updates

### Phase 3: Batch Processing and Verification
- [ ] Implement batch verification logic 
- [ ] Handle turn flag return mechanism when batches are rejected
- [ ] Add proper merkle chaining with salted hashes  
- [ ] Implement tombstone functionality (soft/hard deletes)

## Technical Details

### Turn Flag Management
The turn flag will be:
1. Stored in settings region of file (not base workbook)
2. Managed through Z-layer updates 
3. Handled by poll guard mechanism that prevents conflicting writes
4. Triggered via Ctrl+Enter or slash-menu entry (since Enter commits cell edits)

### Z-Layer Status Handling
- **Accepted**: Layer incorporated into current workbook state  
- **Not Accepted**: Layer discarded but kept in manifest for reference
- **Kept as Note**: Layer stored separately and can be referenced later without applying

### Merkle Chain Implementation
- Salted 256-bit hashes per layer (128-bit salt + 128-bit hash)
- Layers chained using merkle tree structure  
- Hard deletes require chain patches to overwrite data in place
- Soft tombstones for non-sensitive data removal

## Files Modified/Created

### Core Implementation Files:
- `crates/lotus-core/src/model/workbook.rs` - Added Z-layer management fields (manifest, merkle_chain)
- `crates/lotus-core/src/storage/reader.rs` - Updated with z-layer processing functions
- `crates/lotus-core/src/zlayer/mod.rs` - Main Z-layer module 
- `crates/lotus-core/src/zlayer/manifest.rs` - Layer status tracking system
- `crates/lotus-core/src/zlayer/chaining.rs` - Merkle chaining implementation

### New Files Created:
- `crates/lotus-core/src/storage/zlayer.rs` - Z-layer storage handling functions

## Testing Strategy

### Unit Tests:
- [x] Basic manifest functionality (creation, status updates)
- [x] Basic merkle chain functionality
- [x] Workbook structure initialization with new fields
- [ ] Complete AI workflow (direct file, chat, API) 
- [ ] File reload detection of new z-layers
- [ ] UI integration for paste areas

### Integration Tests: 
- [ ] End-to-end Z-layer processing pipeline
- [ ] Turn flag handling between human/AI
- [ ] Batch rejection scenarios

## Backup Strategy
As per project requirements:
1. Full backup of repository before starting implementation  
2. Checkpoint documentation created at each phase
3. All changes properly documented with clear rationale
4. Version control used to track incremental progress 

## Next Steps
Phase 2: Implement the three AI integration paths (direct file modification, chat-based updates, API interface) in that order.