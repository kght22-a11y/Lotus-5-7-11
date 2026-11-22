//! Z-Axis Historical Layers, Provenance, and Tombstone Overlays.
//!
//! As specified in Lotus 5-7-11 Section 3.6:
//! - The Z axis represents discrete historical layers in sequence (a stack of thin sheets).
//! - History is represented as overlays, avoiding duplicate full-workbook snapshots.
//! - Moving along the Z axis selects earlier or later historical states.
//! - Explicit tombstones track deletions so historical restoration remains unambiguous.

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, format, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use crate::model::{Cell, Coord, Sheet};

/// A mutation event recording historical provenance along the Z axis.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoricalRecord {
    pub z: u32,
    pub coord: Coord,
    pub cell: Option<Cell>, // None represents an explicit Tombstone (deletion)
}

/// Manages multi-layered Z-axis history and provenance overlays.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEngine {
    pub current_z: u32,
    pub max_z: u32,
    pub layers: BTreeMap<u32, Vec<HistoricalRecord>>,
}

impl HistoryEngine {
    pub fn new() -> Self {
        Self {
            current_z: 0,
            max_z: 0,
            layers: BTreeMap::new(),
        }
    }

    /// Record a snapshot layer of changed cells and deletions.
    pub fn record_layer(&mut self, changes: Vec<(Coord, Option<Cell>)>) -> Result<u32, &'static str> {
        let z = self.max_z.checked_add(1).ok_or("History Z axis is exhausted")?;
        self.insert_layer(z, changes)?;
        Ok(z)
    }

    /// Insert a serialized layer while preserving its Z value.
    /// Layers must arrive consecutively, and each cell may appear once per layer.
    pub fn insert_layer(
        &mut self,
        z: u32,
        changes: Vec<(Coord, Option<Cell>)>,
    ) -> Result<(), &'static str> {
        let expected_z = self.max_z.checked_add(1).ok_or("History Z axis is exhausted")?;
        if z != expected_z {
            return Err("History layers must have consecutive Z values starting at 1");
        }
        for (i, (coord, cell)) in changes.iter().enumerate() {
            if cell.as_ref().is_some_and(|value| value.coord != *coord) {
                return Err("History cell coordinate does not match its record coordinate");
            }
            if changes[..i].iter().any(|(prior, _)| prior == coord) {
                return Err("A cell may appear only once in a history layer");
            }
        }
        let records = changes
            .into_iter()
            .map(|(coord, cell)| HistoricalRecord { z, coord, cell })
            .collect();
        self.layers.insert(z, records);
        self.max_z = z;
        self.current_z = z;
        Ok(())
    }

    /// Construct a composite `Sheet` view at a specific discrete Z layer.
    ///
    /// Applies overlays sequentially from layer 0 up to `target_z`:
    /// - Unchanged cells persist.
    /// - Changed cells expose their state at or before `target_z`.
    /// - Deletions (tombstones) suppress cell visibility at and after that layer.
    pub fn composite_at_z(&self, base_sheet: &Sheet, target_z: u32) -> Sheet {
        let mut composite_sheet = Sheet::new(&base_sheet.id, format!("{} (Z={})", base_sheet.name, target_z));

        // Start with base sheet cells (layer 0)
        for (_coord, cell) in &base_sheet.cells {
            composite_sheet.set_cell(cell.clone());
        }

        // Apply layers 1..=target_z in chronological order
        for (z, records) in &self.layers {
            if *z > target_z {
                break;
            }
            for rec in records {
                match &rec.cell {
                    Some(updated_cell) => {
                        composite_sheet.set_cell(updated_cell.clone());
                    }
                    None => {
                        // Tombstone: explicitly remove cell from composite view
                        composite_sheet.cells.remove(&rec.coord);
                    }
                }
            }
        }

        composite_sheet
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CellValue;

    #[test]
    fn test_z_axis_history_and_tombstones() {
        let mut base_sheet = Sheet::new("main", "Main");
        // Layer 0: A1 = 10, A2 = 20
        base_sheet.set_cell(Cell::number(Coord::new(0, 0), 10.0));
        base_sheet.set_cell(Cell::number(Coord::new(0, 1), 20.0));

        let mut history = HistoryEngine::new();

        // Layer 1: Update A1 = 50, Add A3 = 30
        let z1 = history.record_layer(vec![
            (Coord::new(0, 0), Some(Cell::number(Coord::new(0, 0), 50.0))),
            (Coord::new(0, 2), Some(Cell::number(Coord::new(0, 2), 30.0))),
        ]).unwrap();
        assert_eq!(z1, 1);

        // Layer 2: Tombstone (delete) A2, Update A3 = 99
        let z2 = history.record_layer(vec![
            (Coord::new(0, 1), None), // Tombstone on A2
            (Coord::new(0, 2), Some(Cell::number(Coord::new(0, 2), 99.0))),
        ]).unwrap();
        assert_eq!(z2, 2);

        // View at Layer 0 (Original state)
        let view_z0 = history.composite_at_z(&base_sheet, 0);
        assert_eq!(view_z0.cells.len(), 2);
        assert_eq!(view_z0.get_cell(Coord::new(0, 0)).unwrap().value, CellValue::Number(10.0));
        assert_eq!(view_z0.get_cell(Coord::new(0, 1)).unwrap().value, CellValue::Number(20.0));
        assert!(view_z0.get_cell(Coord::new(0, 2)).is_none());

        // View at Layer 1 (A1 modified, A3 added, A2 still present)
        let view_z1 = history.composite_at_z(&base_sheet, 1);
        assert_eq!(view_z1.cells.len(), 3);
        assert_eq!(view_z1.get_cell(Coord::new(0, 0)).unwrap().value, CellValue::Number(50.0));
        assert_eq!(view_z1.get_cell(Coord::new(0, 1)).unwrap().value, CellValue::Number(20.0));
        assert_eq!(view_z1.get_cell(Coord::new(0, 2)).unwrap().value, CellValue::Number(30.0));

        // View at Layer 2 (A2 deleted by tombstone, A3 updated to 99)
        let view_z2 = history.composite_at_z(&base_sheet, 2);
        assert_eq!(view_z2.cells.len(), 2);
        assert_eq!(view_z2.get_cell(Coord::new(0, 0)).unwrap().value, CellValue::Number(50.0));
        assert!(view_z2.get_cell(Coord::new(0, 1)).is_none(), "A2 should be tombstoned");
        assert_eq!(view_z2.get_cell(Coord::new(0, 2)).unwrap().value, CellValue::Number(99.0));
    }
}
