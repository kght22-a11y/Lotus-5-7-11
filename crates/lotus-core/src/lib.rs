//! # Lotus 5-7-11 Core
//!
//! A self-contained, OS-agnostic computational spreadsheet engine and structured workspace.
//! Designed for x86-64 execution with `#![no_std]` capability.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod app;
pub mod basic;
pub mod calc;
pub mod doc;
pub mod history;
pub mod memory;
pub mod model;
pub mod storage;
pub mod substrate;
pub mod tui;
pub mod view;

pub use basic::{BasicError, BasicRunner, Turtle};
pub use calc::CalcEngine;
pub use doc::{
    heading_to_slug, BrokenDocCellRef, BrokenDocRef, BrokenSectionRef, DanglingFormulaRef,
    Document, DocumentRefStatus, OrphanReport, SectionHeading,
};
pub use history::HistoryEngine;
pub use memory::{BumpArena, MemoryError, MemoryRegion};
pub use model::{
    Cell, CellValue, ColorRgba, Coord, Point2D, Rect2D, Sheet, StrokeStyle, TableOfContents,
    TocEntry, VectorBuffer, VectorOp, Workbook,
};
pub use storage::{deserialize_workbook, serialize_workbook, ParseError};
pub use substrate::{Partition, SharedMailbox, SubstrateDescriptor};
pub use tui::{MenuAction, MenuItem, MenuState, WorkspaceSession};
pub use app::{App, Event, Host, Key, MemoryHost, Mods, PointerButton};
pub use view::{
    Bitmap, ContextMenu, ContextMenuAction, ContextMenuItem, GridRenderer, HersheyFont,
    HersheyGlyph, StrokeSegment, SubpixelRasterizer, Transform2D, VectorWorkspaceView,
};
