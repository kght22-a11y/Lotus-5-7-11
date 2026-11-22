//! High-level GUI primitives and Context Menu built on top of VectorBuffer and Bitmap.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec, vec::Vec};

use crate::{
    model::{
        coord::Coord,
        vector::{ColorRgba, Rect2D, VectorBuffer},
    },
    tui::menu::MenuAction,
    view::{Bitmap, HersheyFont},
};

pub struct Button {
    pub ops: VectorBuffer,
    pub bounds: Rect2D,
    pub cell_ref: Coord,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContextMenuAction {
    None,
    Copy,
    Paste,
    Cut,
    Clear,
    Lotus(MenuAction),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenuItem {
    pub id: &'static str,
    pub label: &'static str,
    pub shortcut: &'static str,
    pub action: ContextMenuAction,
    pub is_separator: bool,
    pub submenus: Vec<ContextMenuItem>,
}

impl ContextMenuItem {
    pub fn item(label: &'static str, shortcut: &'static str, action: ContextMenuAction) -> Self {
        Self {
            id: label,
            label,
            shortcut,
            action,
            is_separator: false,
            submenus: Vec::new(),
        }
    }

    pub fn with_submenus(label: &'static str, shortcut: &'static str, submenus: Vec<ContextMenuItem>) -> Self {
        Self {
            id: label,
            label,
            shortcut,
            action: ContextMenuAction::None,
            is_separator: false,
            submenus,
        }
    }

    pub fn separator() -> Self {
        Self {
            id: "---",
            label: "",
            shortcut: "",
            action: ContextMenuAction::None,
            is_separator: true,
            submenus: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenu {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub item_height: f32,
    pub is_visible: bool,
    pub items: Vec<ContextMenuItem>,
    pub hovered_idx: Option<usize>,
    pub active_submenu_idx: Option<usize>,
    pub hovered_sub_idx: Option<usize>,
}

impl ContextMenu {
    pub fn new() -> Self {
        Self::default_context_menu()
    }

    /// Build the default Lotus context menu with standard clipboard actions
    /// and the full nested Lotus '/' hierarchical command tree.
    pub fn default_context_menu() -> Self {
        let items = vec![
            ContextMenuItem::item("Cut", "Ctrl+X", ContextMenuAction::Cut),
            ContextMenuItem::item("Copy", "Ctrl+C", ContextMenuAction::Copy),
            ContextMenuItem::item("Paste", "Ctrl+V", ContextMenuAction::Paste),
            ContextMenuItem::item("Clear", "Del", ContextMenuAction::Clear),
            ContextMenuItem::separator(),
            ContextMenuItem::with_submenus(
                "Lotus Commands",
                "/",
                vec![
                    ContextMenuItem::with_submenus(
                        "Worksheet",
                        "/W",
                        vec![
                            ContextMenuItem::item("Recalculate", "/WR", ContextMenuAction::Lotus(MenuAction::RecalculateAll)),
                            ContextMenuItem::item("Orphans Audit", "/WO", ContextMenuAction::Lotus(MenuAction::OrphanCheck)),
                        ],
                    ),
                    ContextMenuItem::with_submenus(
                        "File",
                        "/F",
                        vec![
                            ContextMenuItem::item("Save", "/FS", ContextMenuAction::Lotus(MenuAction::FileSave)),
                            ContextMenuItem::item("Retrieve", "/FR", ContextMenuAction::Lotus(MenuAction::FileReload)),
                            ContextMenuItem::item("Inspect", "/FI", ContextMenuAction::Lotus(MenuAction::FileInspect)),
                        ],
                    ),
                    ContextMenuItem::with_submenus(
                        "Basic",
                        "/B",
                        vec![
                            ContextMenuItem::item("Run Script", "/BR", ContextMenuAction::Lotus(MenuAction::BasicRun)),
                        ],
                    ),
                    ContextMenuItem::with_submenus(
                        "TOC",
                        "/T",
                        vec![
                            ContextMenuItem::item("Jump to TOC", "/TJ", ContextMenuAction::Lotus(MenuAction::TOCJump)),
                        ],
                    ),
                    ContextMenuItem::item("Quit", "/Q", ContextMenuAction::Lotus(MenuAction::Quit)),
                ],
            ),
        ];

        Self {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            item_height: 22.0,
            is_visible: false,
            items,
            hovered_idx: None,
            active_submenu_idx: None,
            hovered_sub_idx: None,
        }
    }

    pub fn open_at(&mut self, x: f32, y: f32) {
        self.x = x.max(0.0);
        self.y = y.max(0.0);
        self.is_visible = true;
        self.hovered_idx = None;
        self.active_submenu_idx = None;
        self.hovered_sub_idx = None;
    }

    pub fn close(&mut self) {
        self.is_visible = false;
        self.hovered_idx = None;
        self.active_submenu_idx = None;
        self.hovered_sub_idx = None;
    }

    pub fn menu_height(&self, items: &[ContextMenuItem]) -> f32 {
        let mut total = 8.0; // padding
        for item in items {
            total += if item.is_separator { 6.0 } else { self.item_height };
        }
        total
    }

    pub fn main_bounds(&self) -> Rect2D {
        let h = self.menu_height(&self.items);
        Rect2D::new(self.x, self.y, self.x + self.width, self.y + h)
    }

    pub fn submenu_bounds(&self) -> Option<(Rect2D, &[ContextMenuItem])> {
        let sub_idx = self.active_submenu_idx?;
        let sub_item = self.items.get(sub_idx)?;
        if sub_item.submenus.is_empty() {
            return None;
        }

        let mut item_y = self.y + 4.0;
        for (i, item) in self.items.iter().enumerate() {
            if i == sub_idx {
                break;
            }
            item_y += if item.is_separator { 6.0 } else { self.item_height };
        }

        let sub_x = self.x + self.width - 4.0;
        let sub_h = self.menu_height(&sub_item.submenus);
        Some((Rect2D::new(sub_x, item_y, sub_x + self.width, item_y + sub_h), &sub_item.submenus))
    }

    /// Update mouse hover position.
    pub fn handle_mouse_move(&mut self, mx: f32, my: f32) {
        if !self.is_visible {
            return;
        }

        // Check if inside active submenu
        if let Some((sub_rect, sub_items)) = self.submenu_bounds() {
            if mx >= sub_rect.min_x && mx <= sub_rect.max_x && my >= sub_rect.min_y && my <= sub_rect.max_y {
                let mut cur_y = sub_rect.min_y + 4.0;
                let mut found_sub = None;
                for (i, item) in sub_items.iter().enumerate() {
                    let h = if item.is_separator { 6.0 } else { self.item_height };
                    if my >= cur_y && my < cur_y + h && !item.is_separator {
                        found_sub = Some(i);
                        break;
                    }
                    cur_y += h;
                }
                self.hovered_sub_idx = found_sub;
                return;
            } else {
                self.hovered_sub_idx = None;
            }
        }

        // Check if inside main menu
        let main = self.main_bounds();
        if mx >= main.min_x && mx <= main.max_x && my >= main.min_y && my <= main.max_y {
            let mut cur_y = self.y + 4.0;
            let mut found_main = None;
            for (i, item) in self.items.iter().enumerate() {
                let h = if item.is_separator { 6.0 } else { self.item_height };
                if my >= cur_y && my < cur_y + h && !item.is_separator {
                    found_main = Some(i);
                    break;
                }
                cur_y += h;
            }
            self.hovered_idx = found_main;

            if let Some(idx) = found_main {
                if !self.items[idx].submenus.is_empty() {
                    self.active_submenu_idx = Some(idx);
                }
            }
        } else {
            self.hovered_idx = None;
        }
    }

    /// Process a mouse click. Returns an action if an actionable item was selected.
    pub fn handle_mouse_click(&mut self, mx: f32, my: f32) -> Option<ContextMenuAction> {
        if !self.is_visible {
            return None;
        }

        // 1. Check Submenu click
        if let Some((sub_rect, sub_items)) = self.submenu_bounds() {
            if mx >= sub_rect.min_x && mx <= sub_rect.max_x && my >= sub_rect.min_y && my <= sub_rect.max_y {
                let mut cur_y = sub_rect.min_y + 4.0;
                for item in sub_items {
                    let h = if item.is_separator { 6.0 } else { self.item_height };
                    if my >= cur_y && my < cur_y + h && !item.is_separator {
                        let action = item.action.clone();
                        if action != ContextMenuAction::None {
                            self.close();
                            return Some(action);
                        }
                    }
                    cur_y += h;
                }
                return None;
            }
        }

        // 2. Check Main Menu click
        let main = self.main_bounds();
        if mx >= main.min_x && mx <= main.max_x && my >= main.min_y && my <= main.max_y {
            let mut cur_y = self.y + 4.0;
            for (i, item) in self.items.iter().enumerate() {
                let h = if item.is_separator { 6.0 } else { self.item_height };
                if my >= cur_y && my < cur_y + h && !item.is_separator {
                    if !item.submenus.is_empty() {
                        self.active_submenu_idx = Some(i);
                        return None;
                    } else {
                        let action = item.action.clone();
                        self.close();
                        return Some(action);
                    }
                }
                cur_y += h;
            }
            return None;
        }

        // Click was outside menu
        self.close();
        None
    }

    /// Render the context menu onto the bitmap and vector overlay buffer.
    pub fn render(&self, bitmap: &mut Bitmap, vec_buf: &mut VectorBuffer, font: &HersheyFont) {
        if !self.is_visible {
            return;
        }

        // 1. Render Main Menu Box
        let main = self.main_bounds();
        let mx = main.min_x as usize;
        let my = main.min_y as usize;
        let mw = main.width() as usize;
        let mh = main.height() as usize;

        // Drop shadow
        bitmap.fill_rect(mx + 3, my + 3, mw, mh, 0x00202020);
        // Beveled menu surface: Classic Windows / OS/2 3D face (0x00D4D0C8)
        bitmap.fill_rect(mx, my, mw, mh, 0x00D4D0C8);

        // 3D Bevel borders
        vec_buf.set_width(1.0);
        // Top & Left highlight
        vec_buf.set_color(ColorRgba::WHITE);
        vec_buf.move_to(main.min_x, main.max_y);
        vec_buf.line_to(main.min_x, main.min_y);
        vec_buf.line_to(main.max_x, main.min_y);

        // Bottom & Right shadow
        vec_buf.set_color(ColorRgba::rgb(64, 64, 64));
        vec_buf.line_to(main.max_x, main.max_y);
        vec_buf.line_to(main.min_x, main.max_y);

        // Items in Main Menu
        let mut cur_y = main.min_y + 4.0;
        for (i, item) in self.items.iter().enumerate() {
            if item.is_separator {
                // 3D separator groove
                let sep_y = (cur_y + 2.0) as usize;
                bitmap.fill_rect(mx + 4, sep_y, mw - 8, 1, 0x00808080);
                bitmap.fill_rect(mx + 4, sep_y + 1, mw - 8, 1, 0x00FFFFFF);
                cur_y += 6.0;
                continue;
            }

            let is_hovered = self.hovered_idx == Some(i) || self.active_submenu_idx == Some(i);
            if is_hovered {
                // Highlight bar: Classic Navy Blue (0x00000080)
                let bar_y = cur_y as usize;
                bitmap.fill_rect(mx + 3, bar_y, mw - 6, self.item_height as usize, 0x00000080);
            }

            let text_color = if is_hovered { ColorRgba::WHITE } else { ColorRgba::BLACK };
            let hint_color = if is_hovered { ColorRgba::WHITE } else { ColorRgba::rgb(96, 96, 96) };

            // Item Label
            vec_buf.set_color(text_color);
            vec_buf.set_width(1.0);
            font.draw_string(item.label, main.min_x + 12.0, cur_y + 15.0, 0.70, vec_buf);

            // Shortcut Hint (Right-Aligned)
            if !item.shortcut.is_empty() {
                let (sw, _) = font.measure_string(item.shortcut, 0.65);
                let shortcut_x = main.max_x - sw - if !item.submenus.is_empty() { 20.0 } else { 12.0 };
                vec_buf.set_color(hint_color);
                font.draw_string(item.shortcut, shortcut_x, cur_y + 15.0, 0.65, vec_buf);
            }

            // Submenu Indicator (">")
            if !item.submenus.is_empty() {
                vec_buf.set_color(text_color);
                font.draw_string(">", main.max_x - 14.0, cur_y + 15.0, 0.70, vec_buf);
            }

            cur_y += self.item_height;
        }

        // 2. Render Submenu if active
        if let Some((sub_rect, sub_items)) = self.submenu_bounds() {
            let sx = sub_rect.min_x as usize;
            let sy = sub_rect.min_y as usize;
            let sw = sub_rect.width() as usize;
            let sh = sub_rect.height() as usize;

            // Drop shadow
            bitmap.fill_rect(sx + 3, sy + 3, sw, sh, 0x00202020);
            // Beveled submenu surface
            bitmap.fill_rect(sx, sy, sw, sh, 0x00D4D0C8);

            // 3D Bevel borders
            vec_buf.set_color(ColorRgba::WHITE);
            vec_buf.move_to(sub_rect.min_x, sub_rect.max_y);
            vec_buf.line_to(sub_rect.min_x, sub_rect.min_y);
            vec_buf.line_to(sub_rect.max_x, sub_rect.min_y);

            vec_buf.set_color(ColorRgba::rgb(64, 64, 64));
            vec_buf.line_to(sub_rect.max_x, sub_rect.max_y);
            vec_buf.line_to(sub_rect.min_x, sub_rect.max_y);

            let mut sub_y = sub_rect.min_y + 4.0;
            for (j, item) in sub_items.iter().enumerate() {
                if item.is_separator {
                    let sep_y = (sub_y + 2.0) as usize;
                    bitmap.fill_rect(sx + 4, sep_y, sw - 8, 1, 0x00808080);
                    bitmap.fill_rect(sx + 4, sep_y + 1, sw - 8, 1, 0x00FFFFFF);
                    sub_y += 6.0;
                    continue;
                }

                let is_sub_hovered = self.hovered_sub_idx == Some(j);
                if is_sub_hovered {
                    let bar_y = sub_y as usize;
                    bitmap.fill_rect(sx + 3, bar_y, sw - 6, self.item_height as usize, 0x00000080);
                }

                let text_color = if is_sub_hovered { ColorRgba::WHITE } else { ColorRgba::BLACK };
                let hint_color = if is_sub_hovered { ColorRgba::WHITE } else { ColorRgba::rgb(96, 96, 96) };

                vec_buf.set_color(text_color);
                vec_buf.set_width(1.0);
                font.draw_string(item.label, sub_rect.min_x + 12.0, sub_y + 15.0, 0.70, vec_buf);

                if !item.shortcut.is_empty() {
                    let (hw, _) = font.measure_string(item.shortcut, 0.65);
                    let shortcut_x = sub_rect.max_x - hw - 12.0;
                    vec_buf.set_color(hint_color);
                    font.draw_string(item.shortcut, shortcut_x, sub_y + 15.0, 0.65, vec_buf);
                }

                sub_y += self.item_height;
            }
        }
    }
}

pub struct GeoWorks;

impl GeoWorks {
    /// Renders a classic beveled window.
    pub fn window(x: f32, y: f32, w: f32, h: f32, title: &str, font: &HersheyFont) -> VectorBuffer {
        let mut buf = VectorBuffer::new();

        // Outer Border (Shadow/Light effect)
        buf.set_color(ColorRgba::rgb(0, 0, 0));
        buf.set_width(2.0);
        buf.rect(x, y, w, h);

        buf.set_color(ColorRgba::rgb(255, 255, 255));
        buf.rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0);

        // Title Bar
        buf.set_color(ColorRgba::rgb(0, 0, 128));
        buf.set_width(1.0);
        buf.rect(x + 2.0, y + 2.0, w - 4.0, 20.0);

        // Title Text
        buf.set_color(ColorRgba::WHITE);
        buf.set_width(1.0);
        font.draw_string(title, x + 10.0, y + 14.0, 0.8, &mut buf);

        buf
    }

    /// Renders a beveled button.
    pub fn button(x: f32, y: f32, w: f32, h: f32, label: &str, font: &HersheyFont, cell: Coord) -> Button {
        let mut buf = VectorBuffer::new();

        buf.set_color(ColorRgba::rgb(0, 0, 0));
        buf.set_width(1.0);
        buf.rect(x, y, w, h);

        buf.set_color(ColorRgba::rgb(255, 255, 255));
        buf.rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0);

        buf.set_color(ColorRgba::BLACK);
        buf.set_width(1.0);
        font.draw_string(label, x + (w / 2.0) - 20.0, y + (h / 2.0), 0.8, &mut buf);

        Button {
            ops: buf,
            bounds: Rect2D::new(x, y, x + w, y + h),
            cell_ref: cell,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_menu_default_structure() {
        let menu = ContextMenu::default_context_menu();
        assert_eq!(menu.items.len(), 6);
        assert_eq!(menu.items[0].label, "Cut");
        assert_eq!(menu.items[0].shortcut, "Ctrl+X");
        assert_eq!(menu.items[1].label, "Copy");
        assert_eq!(menu.items[1].shortcut, "Ctrl+C");
        assert_eq!(menu.items[2].label, "Paste");
        assert_eq!(menu.items[2].shortcut, "Ctrl+V");
        assert_eq!(menu.items[3].label, "Clear");
        assert_eq!(menu.items[3].shortcut, "Del");
        assert!(menu.items[4].is_separator);
        assert_eq!(menu.items[5].label, "Lotus Commands");
        assert_eq!(menu.items[5].submenus.len(), 5);

        // Check nested File -> Save (/FS)
        let file_menu = menu.items[5].submenus.iter().find(|m| m.label == "File").unwrap();
        let save_item = file_menu.submenus.iter().find(|m| m.label == "Save").unwrap();
        assert_eq!(save_item.shortcut, "/FS");
        assert_eq!(save_item.action, ContextMenuAction::Lotus(MenuAction::FileSave));
    }

    #[test]
    fn test_context_menu_mouse_interaction() {
        let mut menu = ContextMenu::default_context_menu();
        menu.open_at(100.0, 100.0);
        assert!(menu.is_visible);

        // Hover over "Copy" item (item 1 at y = 100 + 4 + 22 = 126)
        menu.handle_mouse_move(150.0, 130.0);
        assert_eq!(menu.hovered_idx, Some(1));

        // Click on "Copy"
        let action = menu.handle_mouse_click(150.0, 130.0);
        assert_eq!(action, Some(ContextMenuAction::Copy));
        assert!(!menu.is_visible); // Closes on action

        // Reopen and click outside
        menu.open_at(100.0, 100.0);
        let outside_action = menu.handle_mouse_click(10.0, 10.0);
        assert_eq!(outside_action, None);
        assert!(!menu.is_visible);
    }
}
