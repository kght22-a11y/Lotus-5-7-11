//! High-level GUI primitives and Context Menu built on top of VectorBuffer and Bitmap.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec, vec::Vec};

use crate::{
    model::{
        coord::Coord,
        vector::{ColorRgba, Rect2D, VectorBuffer},
    },
    tui::menu::{default_lotus_menu, MenuAction, MenuItem},
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
    pub shortcut: String,
    pub action: ContextMenuAction,
    pub is_separator: bool,
    pub submenus: Vec<ContextMenuItem>,
}

impl ContextMenuItem {
    pub fn item(label: &'static str, shortcut: impl Into<String>, action: ContextMenuAction) -> Self {
        Self {
            id: label,
            label,
            shortcut: shortcut.into(),
            action,
            is_separator: false,
            submenus: Vec::new(),
        }
    }

    pub fn with_submenus(label: &'static str, shortcut: impl Into<String>, submenus: Vec<ContextMenuItem>) -> Self {
        Self {
            id: label,
            label,
            shortcut: shortcut.into(),
            action: ContextMenuAction::None,
            is_separator: false,
            submenus,
        }
    }

    pub fn separator() -> Self {
        Self {
            id: "---",
            label: "",
            shortcut: String::new(),
            action: ContextMenuAction::None,
            is_separator: true,
            submenus: Vec::new(),
        }
    }

    /// Convert a node of the `/` command tree. `prefix` is the slash path of
    /// the parent (`"/"` at the root); the item's own shortcut is that path
    /// plus its hotkey, so `File -> Save` becomes `/FS` without anyone typing it.
    pub fn from_lotus(item: &MenuItem, prefix: &str) -> Self {
        let mut shortcut = String::from(prefix);
        shortcut.push(item.key);
        if item.submenus.is_empty() {
            Self::item(item.label, shortcut, ContextMenuAction::Lotus(item.action.clone()))
        } else {
            let subs = item.submenus.iter().map(|s| Self::from_lotus(s, &shortcut)).collect();
            Self::with_submenus(item.label, shortcut, subs)
        }
    }
}

/// One open column of the menu: the root list, or the submenu of the item
/// highlighted in the column before it.
#[derive(Debug, Clone)]
pub struct Panel<'a> {
    pub depth: usize,
    pub rect: Rect2D,
    pub items: &'a [ContextMenuItem],
    pub highlighted: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenu {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub item_height: f32,
    pub is_visible: bool,
    pub items: Vec<ContextMenuItem>,
    /// Highlighted item at each open level, root first. A level is open when
    /// the item highlighted on the level before it has submenus, so this one
    /// list drives hover, click and rendering at any depth.
    pub path: Vec<usize>,
    /// Window size used to keep panels on screen. `(0.0, 0.0)` means unconstrained.
    pub viewport: (f32, f32),
}

impl ContextMenu {
    pub fn new() -> Self {
        Self::default_context_menu()
    }

    /// Standard clipboard actions plus the full nested Lotus `/` command tree.
    /// The command part is generated from [`default_lotus_menu`], so the two
    /// menus cannot drift apart.
    pub fn default_context_menu() -> Self {
        Self::from_lotus_menu(&default_lotus_menu())
    }

    /// Build a context menu whose "Lotus Commands" submenu mirrors `root`.
    pub fn from_lotus_menu(root: &[MenuItem]) -> Self {
        let commands = root.iter().map(|m| ContextMenuItem::from_lotus(m, "/")).collect();
        let items = vec![
            ContextMenuItem::item("Cut", "Ctrl+X", ContextMenuAction::Cut),
            ContextMenuItem::item("Copy", "Ctrl+C", ContextMenuAction::Copy),
            ContextMenuItem::item("Paste", "Ctrl+V", ContextMenuAction::Paste),
            ContextMenuItem::item("Clear", "Del", ContextMenuAction::Clear),
            ContextMenuItem::separator(),
            ContextMenuItem::with_submenus("Lotus Commands", "/", commands),
        ];

        Self {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            item_height: 22.0,
            is_visible: false,
            items,
            path: Vec::new(),
            viewport: (0.0, 0.0),
        }
    }

    /// Tell the menu how big the window is so panels can flip and shift to stay visible.
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.viewport = (width, height);
    }

    pub fn open_at(&mut self, x: f32, y: f32) {
        let (vw, vh) = self.viewport;
        let h = self.menu_height(&self.items);
        let mut x = x.max(0.0);
        let mut y = y.max(0.0);
        if vw > 0.0 && x + self.width > vw {
            x = (vw - self.width).max(0.0);
        }
        if vh > 0.0 && y + h > vh {
            y = (vh - h).max(0.0);
        }
        self.x = x;
        self.y = y;
        self.is_visible = true;
        self.path.clear();
    }

    pub fn close(&mut self) {
        self.is_visible = false;
        self.path.clear();
    }

    fn row_height(&self, item: &ContextMenuItem) -> f32 {
        if item.is_separator { 6.0 } else { self.item_height }
    }

    pub fn menu_height(&self, items: &[ContextMenuItem]) -> f32 {
        let mut total = 8.0; // padding
        for item in items {
            total += self.row_height(item);
        }
        total
    }

    pub fn main_bounds(&self) -> Rect2D {
        let h = self.menu_height(&self.items);
        Rect2D::new(self.x, self.y, self.x + self.width, self.y + h)
    }

    /// Item under the pointer in the root list, if any (kept for callers that
    /// only care about the first level).
    pub fn hovered_idx(&self) -> Option<usize> {
        self.path.first().copied()
    }

    /// Root item whose submenu is currently open, if any.
    pub fn active_submenu_idx(&self) -> Option<usize> {
        let i = *self.path.first()?;
        if self.items.get(i)?.submenus.is_empty() { None } else { Some(i) }
    }

    /// Every open panel, root first. Submenus open to the right of their
    /// parent, flip to its left edge when they would leave the window, and
    /// shift up when they would run off the bottom.
    pub fn panels(&self) -> Vec<Panel<'_>> {
        let mut out = Vec::new();
        if !self.is_visible {
            return out;
        }
        let main = self.main_bounds();
        let mut rect = main;
        let mut items: &[ContextMenuItem] = &self.items;
        let mut depth = 0;
        loop {
            let highlighted = self.path.get(depth).copied().filter(|&i| i < items.len());
            out.push(Panel { depth, rect, items, highlighted });

            let Some(hi) = highlighted else { break };
            let item = &items[hi];
            if item.submenus.is_empty() {
                break;
            }

            let mut item_y = rect.min_y + 4.0;
            for it in &items[..hi] {
                item_y += self.row_height(it);
            }
            let mut nx = rect.max_x - 4.0;
            if self.viewport.0 > 0.0 && nx + self.width > self.viewport.0 {
                nx = (rect.min_x - self.width + 4.0).max(0.0);
            }
            let nh = self.menu_height(&item.submenus);
            let mut ny = item_y;
            if self.viewport.1 > 0.0 && ny + nh > self.viewport.1 {
                ny = (self.viewport.1 - nh).max(0.0);
            }
            rect = Rect2D::new(nx, ny, nx + self.width, ny + nh);
            items = &item.submenus;
            depth += 1;
        }
        out
    }

    /// The first submenu panel, if one is open.
    pub fn submenu_bounds(&self) -> Option<(Rect2D, &[ContextMenuItem])> {
        self.panels().into_iter().nth(1).map(|p| (p.rect, p.items))
    }

    fn item_at(&self, panel: &Panel<'_>, my: f32) -> Option<usize> {
        let mut cur_y = panel.rect.min_y + 4.0;
        for (i, item) in panel.items.iter().enumerate() {
            let h = self.row_height(item);
            if my >= cur_y && my < cur_y + h && !item.is_separator {
                return Some(i);
            }
            cur_y += h;
        }
        None
    }

    /// Deepest panel under the point, with the item under it (None over padding or a separator).
    fn hit(&self, mx: f32, my: f32) -> Option<(usize, Option<usize>)> {
        for p in self.panels().iter().rev() {
            let r = &p.rect;
            if mx >= r.min_x && mx <= r.max_x && my >= r.min_y && my <= r.max_y {
                return Some((p.depth, self.item_at(p, my)));
            }
        }
        None
    }

    /// Update the highlight path from the pointer position.
    pub fn handle_mouse_move(&mut self, mx: f32, my: f32) {
        if !self.is_visible {
            return;
        }
        if let Some((depth, idx)) = self.hit(mx, my) {
            self.path.truncate(depth);
            if let Some(i) = idx {
                self.path.push(i);
            }
        }
    }

    /// Process a mouse click. Returns an action if an actionable item was selected.
    /// Clicking a group opens it; clicking outside every panel closes the menu.
    pub fn handle_mouse_click(&mut self, mx: f32, my: f32) -> Option<ContextMenuAction> {
        if !self.is_visible {
            return None;
        }
        let Some((depth, idx)) = self.hit(mx, my) else {
            self.close();
            return None;
        };
        let i = idx?;

        let (has_submenus, action) = {
            let panels = self.panels();
            let item = &panels[depth].items[i];
            (!item.submenus.is_empty(), item.action.clone())
        };
        self.path.truncate(depth);
        self.path.push(i);

        if has_submenus || action == ContextMenuAction::None {
            return None;
        }
        self.close();
        Some(action)
    }

    /// Render every open panel onto the bitmap and vector overlay buffer.
    pub fn render(&self, bitmap: &mut Bitmap, vec_buf: &mut VectorBuffer, font: &HersheyFont) {
        for panel in self.panels() {
            self.draw_panel(&panel, bitmap, vec_buf, font);
        }
    }

    fn draw_panel(&self, panel: &Panel<'_>, bitmap: &mut Bitmap, vec_buf: &mut VectorBuffer, font: &HersheyFont) {
        let r = panel.rect;
        let px = r.min_x as usize;
        let py = r.min_y as usize;
        let pw = r.width() as usize;
        let ph = r.height() as usize;

        // Drop shadow, then the classic 3D face (0x00D4D0C8)
        bitmap.fill_rect(px + 3, py + 3, pw, ph, 0x00202020);
        bitmap.fill_rect(px, py, pw, ph, 0x00D4D0C8);

        // 3D bevel: white top/left, dark bottom/right
        vec_buf.set_width(1.0);
        vec_buf.set_color(ColorRgba::WHITE);
        vec_buf.move_to(r.min_x, r.max_y);
        vec_buf.line_to(r.min_x, r.min_y);
        vec_buf.line_to(r.max_x, r.min_y);
        vec_buf.set_color(ColorRgba::rgb(64, 64, 64));
        vec_buf.line_to(r.max_x, r.max_y);
        vec_buf.line_to(r.min_x, r.max_y);

        let mut cur_y = r.min_y + 4.0;
        for (i, item) in panel.items.iter().enumerate() {
            if item.is_separator {
                let sep_y = (cur_y + 2.0) as usize;
                bitmap.fill_rect(px + 4, sep_y, pw.saturating_sub(8), 1, 0x00808080);
                bitmap.fill_rect(px + 4, sep_y + 1, pw.saturating_sub(8), 1, 0x00FFFFFF);
                cur_y += 6.0;
                continue;
            }

            let is_hot = panel.highlighted == Some(i);
            if is_hot {
                // Highlight bar: classic navy blue
                bitmap.fill_rect(px + 3, cur_y as usize, pw.saturating_sub(6), self.item_height as usize, 0x00000080);
            }
            let text_color = if is_hot { ColorRgba::WHITE } else { ColorRgba::BLACK };
            let hint_color = if is_hot { ColorRgba::WHITE } else { ColorRgba::rgb(96, 96, 96) };

            vec_buf.set_color(text_color);
            vec_buf.set_width(1.0);
            font.draw_string(item.label, r.min_x + 12.0, cur_y + 15.0, 0.70, vec_buf);

            if !item.shortcut.is_empty() {
                let (sw, _) = font.measure_string(&item.shortcut, 0.65);
                let margin = if item.submenus.is_empty() { 12.0 } else { 20.0 };
                vec_buf.set_color(hint_color);
                font.draw_string(&item.shortcut, r.max_x - sw - margin, cur_y + 15.0, 0.65, vec_buf);
            }

            if !item.submenus.is_empty() {
                vec_buf.set_color(text_color);
                font.draw_string(">", r.max_x - 14.0, cur_y + 15.0, 0.70, vec_buf);
            }

            cur_y += self.item_height;
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

    /// Vertical centre of row `idx` in a panel with no separators.
    fn row_y(rect: Rect2D, idx: usize, item_height: f32) -> f32 {
        rect.min_y + 4.0 + idx as f32 * item_height + item_height / 2.0
    }

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
        assert_eq!(menu.hovered_idx(), Some(1));

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

    #[test]
    fn nested_commands_are_reachable_by_mouse_at_every_level() {
        let mut menu = ContextMenu::default_context_menu();
        menu.open_at(80.0, 120.0);

        // Level 0 -> "Lotus Commands" (row 5, after a 6px separator)
        let p0 = menu.panels()[0].rect;
        let lotus_y = p0.min_y + 4.0 + 4.0 * 22.0 + 6.0 + 11.0;
        menu.handle_mouse_move(100.0, lotus_y);
        assert_eq!(menu.path, vec![5]);
        assert_eq!(menu.panels().len(), 2);

        // Level 1 -> "File" (row 1)
        let p1 = menu.panels()[1].rect;
        menu.handle_mouse_move(p1.min_x + 20.0, row_y(p1, 1, 22.0));
        assert_eq!(menu.path, vec![5, 1]);
        assert_eq!(menu.panels().len(), 3, "File's submenu must open as a third panel");

        // Level 2 -> "Save" (row 0) triggers /FS
        let p2 = menu.panels()[2].rect;
        assert!(p2.min_x > p1.min_x);
        let action = menu.handle_mouse_click(p2.min_x + 20.0, row_y(p2, 0, 22.0));
        assert_eq!(action, Some(ContextMenuAction::Lotus(MenuAction::FileSave)));
        assert!(!menu.is_visible);
    }

    #[test]
    fn clicking_a_group_opens_it_without_closing_the_menu() {
        let mut menu = ContextMenu::default_context_menu();
        menu.open_at(80.0, 120.0);
        let p0 = menu.panels()[0].rect;
        let lotus_y = p0.min_y + 4.0 + 4.0 * 22.0 + 6.0 + 11.0;

        assert_eq!(menu.handle_mouse_click(100.0, lotus_y), None);
        assert!(menu.is_visible);
        assert_eq!(menu.active_submenu_idx(), Some(5));

        let p1 = menu.panels()[1].rect;
        assert_eq!(menu.handle_mouse_click(p1.min_x + 20.0, row_y(p1, 0, 22.0)), None);
        assert!(menu.is_visible);
        assert_eq!(menu.path, vec![5, 0]);
        assert_eq!(menu.panels().len(), 3);
    }

    #[test]
    fn hovering_a_plain_item_closes_open_submenus() {
        let mut menu = ContextMenu::default_context_menu();
        menu.open_at(80.0, 120.0);
        let p0 = menu.panels()[0].rect;
        menu.handle_mouse_move(100.0, p0.min_y + 4.0 + 4.0 * 22.0 + 6.0 + 11.0);
        assert_eq!(menu.panels().len(), 2);

        menu.handle_mouse_move(100.0, row_y(p0, 1, 22.0)); // Copy
        assert_eq!(menu.path, vec![1]);
        assert_eq!(menu.panels().len(), 1);
    }

    #[test]
    fn panels_stay_inside_the_viewport() {
        let mut menu = ContextMenu::default_context_menu();
        menu.set_viewport(800.0, 600.0);
        menu.open_at(790.0, 590.0);

        let main = menu.main_bounds();
        assert!(main.max_x <= 800.0 && main.max_y <= 600.0, "root panel clamped: {main:?}");

        let p0 = menu.panels()[0].rect;
        menu.handle_mouse_move(p0.min_x + 20.0, p0.min_y + 4.0 + 4.0 * 22.0 + 6.0 + 11.0);
        let p1 = menu.panels()[1].rect;
        assert!(p1.max_x <= 800.0 && p1.min_x >= 0.0);
        assert!(p1.max_y <= 600.0);
        assert!(p1.max_x <= p0.max_x, "submenu flips to the left of its parent near the right edge");
    }

    #[test]
    fn context_menu_is_generated_from_the_command_tree() {
        let tree = vec![
            MenuItem::new('T', "Tools", "", MenuAction::None).with_submenus(vec![
                MenuItem::new('X', "Export", "", MenuAction::FileInspect),
            ]),
        ];
        let menu = ContextMenu::from_lotus_menu(&tree);
        let commands = &menu.items[5].submenus;
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].shortcut, "/T");
        assert_eq!(commands[0].submenus[0].shortcut, "/TX");
        assert_eq!(commands[0].submenus[0].action, ContextMenuAction::Lotus(MenuAction::FileInspect));
    }

    #[test]
    fn every_slash_command_appears_in_the_context_menu() {
        fn leaves(items: &[MenuItem], out: &mut Vec<MenuAction>) {
            for i in items {
                if i.submenus.is_empty() { out.push(i.action.clone()); } else { leaves(&i.submenus, out); }
            }
        }
        fn ctx_leaves(items: &[ContextMenuItem], out: &mut Vec<MenuAction>) {
            for i in items {
                if i.submenus.is_empty() {
                    if let ContextMenuAction::Lotus(a) = &i.action { out.push(a.clone()); }
                } else { ctx_leaves(&i.submenus, out); }
            }
        }
        let mut slash = Vec::new();
        leaves(&default_lotus_menu(), &mut slash);
        let mut ctx = Vec::new();
        ctx_leaves(&ContextMenu::default_context_menu().items, &mut ctx);
        assert_eq!(slash, ctx);
    }
}
