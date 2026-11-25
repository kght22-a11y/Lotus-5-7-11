//! The whole OS surface Lotus needs, as one small trait.
//!
//! Everything else (drawing, input handling, menus, calculation, storage
//! format) lives in `lotus-core` and is OS-agnostic. A platform shell supplies:
//!
//! 1. a pixel surface to blit [`App::render`](super::App::render) onto,
//! 2. a translation of native input into [`Event`](super::Event)s,
//! 3. an implementation of this trait.
//!
//! Calls are synchronous on purpose: a bare-metal host can answer them
//! straight from its own filesystem and clipboard without an event loop.

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

pub trait Host {
    /// Text currently on the system clipboard, if any.
    fn clipboard_get(&mut self) -> Option<String>;
    /// Replace the system clipboard contents.
    fn clipboard_set(&mut self, text: &str);
    /// Whole-file read. `None` when the file is missing or unreadable.
    fn read_file(&mut self, path: &str) -> Option<Vec<u8>>;
    /// Whole-file write. The error text is shown to the user as-is.
    fn write_file(&mut self, path: &str, bytes: &[u8]) -> Result<(), String>;
    /// The user asked to quit; the shell should end its loop.
    fn request_quit(&mut self);
}

/// An in-memory [`Host`]: a clipboard string, a map of files and a quit flag.
///
/// Used by the test suite, and handy for headless runs and for bringing up a
/// new platform before its real filesystem and clipboard exist.
#[derive(Debug, Default, Clone)]
pub struct MemoryHost {
    pub clipboard: Option<String>,
    pub files: BTreeMap<String, Vec<u8>>,
    pub quit_requested: bool,
    /// When set, every `write_file` fails (to exercise error reporting).
    pub fail_writes: bool,
}

impl MemoryHost {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Host for MemoryHost {
    fn clipboard_get(&mut self) -> Option<String> {
        self.clipboard.clone()
    }

    fn clipboard_set(&mut self, text: &str) {
        self.clipboard = Some(String::from(text));
    }

    fn read_file(&mut self, path: &str) -> Option<Vec<u8>> {
        self.files.get(path).cloned()
    }

    fn write_file(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
        if self.fail_writes {
            return Err(String::from("disk full"));
        }
        self.files.insert(String::from(path), bytes.to_vec());
        Ok(())
    }

    fn request_quit(&mut self) {
        self.quit_requested = true;
    }
}
