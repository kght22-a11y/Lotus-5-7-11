# Implementation Plan: Vector-Based Font Scaling for Lotus UI

## Overview
Implement granular, vector-based font scaling that can scale individual UI elements based on content rather than whole layers/screens. This will support Z-layer update display and other dynamic text components in the Lotus interface.

## Objectives
1. Create reusable font scaling utilities 
2. Implement automatic sizing for Z-layer paste areas
3. Support vector-based rendering with proper text layout calculations
4. Ensure compatibility with existing VectorBuffer system

## Implementation Approach
- Build on existing `VectorBuffer` and `HersheyFont` infrastructure  
- Create utility functions for calculating optimal font sizes based on available space
- Implement component-level scaling that doesn't require full UI reflows
- Add validation tests to ensure correct behavior

## Files to Modify/Create
1. New utility module: `crates/lotus-core/src/view/font_scaling.rs`
2. Integration with existing GUI components in `crates/lotus-core/src/view/gui.rs`

## Testing Strategy
- Unit tests for font calculation functions
- Integration tests with UI component rendering 
- Edge case handling (empty content, very long text, etc.)