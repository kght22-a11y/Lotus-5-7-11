//! Platform-neutral input events.
//!
//! A shell (Win32 window, X11, a bare-metal keyboard/mouse driver, a test)
//! translates whatever its OS delivers into these and feeds them to
//! [`App::handle_event`](super::App::handle_event). Nothing here knows about
//! virtual-key codes, message loops or scancodes.

/// Keys that act as commands rather than text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Backspace,
    Delete,
    /// A letter pressed as a chord, e.g. `Ctrl+S` arrives as `Key::Char('s')`
    /// with `mods.ctrl` set. Shells send the lowercase letter. Plain typing
    /// does not use this: it arrives as [`Event::Text`].
    Char(char),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Mods {
    pub const NONE: Mods = Mods { ctrl: false, shift: false, alt: false };
    pub const CTRL: Mods = Mods { ctrl: true, shift: false, alt: false };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// The drawable surface changed size, in pixels.
    Resize { width: usize, height: usize },
    /// A command key or chord.
    Key { key: Key, mods: Mods },
    /// A typed character (already layout- and IME-resolved by the shell).
    Text(char),
    /// Pointer position in surface pixels, origin top-left.
    PointerMove { x: f32, y: f32 },
    PointerDown { x: f32, y: f32, button: PointerButton },
}
