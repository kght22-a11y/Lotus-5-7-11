//! Lotus 1-2-3 hierarchical slash-command menu state machine.
//!
//! Pressing '/' opens the top-level menu bar.
//! Submenus appear on line 2 with instant keyboard-shortcut jumping (first letter),
//! arrow-key navigation, and Escape key backtracking.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec, vec::Vec};

#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    None,
    FileSave,
    FileReload,
    FileInspect,
    BasicRun,
    RecalculateAll,
    TOCJump,
    Quit,
    OrphanCheck,
    // Edit
    EditUndo,
    EditRedo,
    EditCut,
    EditCopy,
    EditPaste,
    EditClear,
    EditSelectAll,
    EditFind,
    EditFindNext,
    EditGoTo,
    // Columns and rows
    InsertColumn,
    DeleteColumn,
    ColumnWidth,
    ColumnFit,
    ColumnCollapse,
    ColumnExpand,
    InsertRow,
    DeleteRow,
    RowHeight,
    RowCollapse,
    RowExpand,
    // Data
    FillDown,
    FillRight,
    SortAscending,
    SortDescending,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub key: char,
    pub label: &'static str,
    pub description: &'static str,
    pub action: MenuAction,
    pub submenus: Vec<MenuItem>,
}

impl MenuItem {
    pub fn new(key: char, label: &'static str, description: &'static str, action: MenuAction) -> Self {
        Self {
            key: key.to_ascii_uppercase(),
            label,
            description,
            action,
            submenus: Vec::new(),
        }
    }

    pub fn with_submenus(mut self, submenus: Vec<MenuItem>) -> Self {
        self.submenus = submenus;
        self
    }
}

pub struct MenuState {
    pub is_active: bool,
    pub breadcrumbs: Vec<usize>, // Selected index at each menu depth
    pub root: Vec<MenuItem>,
}

impl MenuState {
    pub fn new() -> Self {
        Self::with_root(default_lotus_menu())
    }

    /// Build a menu state over a custom tree. In debug builds the tree is
    /// checked with [`validate_menu`] so a key collision fails loudly instead
    /// of silently shadowing an item.
    pub fn with_root(root: Vec<MenuItem>) -> Self {
        debug_assert!(
            validate_menu(&root).is_ok(),
            "invalid menu tree: {:?}",
            validate_menu(&root)
        );
        Self {
            is_active: false,
            breadcrumbs: Vec::new(),
            root,
        }
    }

    /// Open or toggle the menu bar with the classic '/' key.
    pub fn open(&mut self) {
        self.is_active = true;
        self.breadcrumbs.clear();
        self.breadcrumbs.push(0);
    }

    pub fn close(&mut self) {
        self.is_active = false;
        self.breadcrumbs.clear();
    }

    /// Handle a key event when the menu is active.
    /// Returns an executed `MenuAction` if an action was triggered.
    pub fn handle_key(&mut self, key_char: Option<char>, is_left: bool, is_right: bool, is_enter: bool, is_esc: bool) -> MenuAction {
        if !self.is_active {
            return MenuAction::None;
        }

        // Escape: Go up one level or dismiss menu
        if is_esc {
            if self.breadcrumbs.len() > 1 {
                self.breadcrumbs.pop();
            } else {
                self.close();
            }
            return MenuAction::None;
        }

        let current_items = self.current_level_items();
        let current_idx = *self.breadcrumbs.last().unwrap_or(&0);

        if is_left {
            let next_idx = if current_idx == 0 {
                current_items.len().saturating_sub(1)
            } else {
                current_idx - 1
            };
            if let Some(last) = self.breadcrumbs.last_mut() {
                *last = next_idx;
            }
            return MenuAction::None;
        }

        if is_right {
            let next_idx = (current_idx + 1) % current_items.len().max(1);
            if let Some(last) = self.breadcrumbs.last_mut() {
                *last = next_idx;
            }
            return MenuAction::None;
        }

        // First-letter shortcut jump (e.g. 'F' for File, 'S' for Save)
        if let Some(c) = key_char {
            let upper = c.to_ascii_uppercase();
            if let Some(pos) = current_items.iter().position(|item| item.key == upper) {
                if let Some(last) = self.breadcrumbs.last_mut() {
                    *last = pos;
                }
                return self.activate_current();
            }
        }

        if is_enter {
            return self.activate_current();
        }

        MenuAction::None
    }

    fn activate_current(&mut self) -> MenuAction {
        let current_idx = *self.breadcrumbs.last().unwrap_or(&0);
        let items = self.current_level_items();

        if let Some(selected) = items.get(current_idx) {
            if !selected.submenus.is_empty() {
                self.breadcrumbs.push(0);
                MenuAction::None
            } else {
                let action = selected.action.clone();
                if action != MenuAction::None {
                    self.close();
                }
                action
            }
        } else {
            MenuAction::None
        }
    }

    pub fn current_level_items(&self) -> &[MenuItem] {
        if self.breadcrumbs.is_empty() {
            return &self.root;
        }

        let mut current = &self.root;
        for &idx in &self.breadcrumbs[..self.breadcrumbs.len() - 1] {
            if let Some(item) = current.get(idx) {
                current = &item.submenus;
            } else {
                return &[];
            }
        }
        current
    }

    /// Returns (Line 1: Menu Items string, Line 2: Explanation/Submenu string).
    pub fn render_lines(&self) -> (String, String) {
        if !self.is_active {
            return (String::new(), String::new());
        }

        let items = self.current_level_items();
        let selected_idx = *self.breadcrumbs.last().unwrap_or(&0);

        let mut line1 = String::new();
        for (i, item) in items.iter().enumerate() {
            if i == selected_idx {
                line1.push_str(&format!("[{}] ", item.label));
            } else {
                line1.push_str(&format!(" {}  ", item.label));
            }
        }

        let line2 = if let Some(selected) = items.get(selected_idx) {
            if !selected.submenus.is_empty() {
                let mut sub_preview = String::new();
                for sub in &selected.submenus {
                    sub_preview.push_str(&format!("{} ", sub.label));
                }
                format!("{}: {}", selected.description, sub_preview)
            } else {
                selected.description.into()
            }
        } else {
            String::new()
        };

        (line1, line2)
    }
}

/// Check a menu tree for mistakes that would otherwise surface as a dead or
/// shadowed item at runtime:
///
/// - two items on the same level share a hotkey (first-letter jump takes the
///   first match, so the second item could never be reached by keyboard);
/// - a group (has submenus) also carries an action, or a leaf has none.
///
/// The error names the slash-path of the offender, e.g. `/WR`.
pub fn validate_menu(items: &[MenuItem]) -> Result<(), String> {
    validate_level(items, "/")
}

fn validate_level(items: &[MenuItem], prefix: &str) -> Result<(), String> {
    for (i, item) in items.iter().enumerate() {
        let mut path = String::from(prefix);
        path.push(item.key);

        if let Some(other) = items[i + 1..].iter().find(|o| o.key == item.key) {
            return Err(format!(
                "duplicate hotkey '{}' at {}: \"{}\" and \"{}\"",
                item.key, prefix, item.label, other.label
            ));
        }

        if item.submenus.is_empty() {
            if item.action == MenuAction::None {
                return Err(format!("{} \"{}\" has neither an action nor submenus", path, item.label));
            }
        } else {
            if item.action != MenuAction::None {
                return Err(format!("{} \"{}\" is a group but also carries an action", path, item.label));
            }
            validate_level(&item.submenus, &path)?;
        }
    }
    Ok(())
}

pub fn default_lotus_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::new('W', "Worksheet", "Global spreadsheet settings and actions", MenuAction::None)
            .with_submenus(vec![
                MenuItem::new('R', "Recalculate", "Recalculate all formulas across sheets", MenuAction::RecalculateAll),
                MenuItem::new('O', "Orphans", "Audit workbook for broken document links and dangling formulas", MenuAction::OrphanCheck),
            ]),
        MenuItem::new('E', "Edit", "Undo, clipboard, selection, find and go to", MenuAction::None)
            .with_submenus(vec![
                MenuItem::new('U', "Undo", "Undo the last change (Ctrl+Z)", MenuAction::EditUndo),
                MenuItem::new('R', "Redo", "Redo the change just undone (Ctrl+Y)", MenuAction::EditRedo),
                MenuItem::new('X', "Cut", "Copy the selection and clear it (Ctrl+X)", MenuAction::EditCut),
                MenuItem::new('C', "Copy", "Copy the selection to the clipboard (Ctrl+C)", MenuAction::EditCopy),
                MenuItem::new('P', "Paste", "Paste the clipboard at the cursor (Ctrl+V)", MenuAction::EditPaste),
                MenuItem::new('D', "Delete", "Clear the selected cells (Del)", MenuAction::EditClear),
                MenuItem::new('A', "Select-all", "Select every used cell (Ctrl+A)", MenuAction::EditSelectAll),
                MenuItem::new('F', "Find", "Find text in values, formulas and names (Ctrl+F)", MenuAction::EditFind),
                MenuItem::new('N', "Next", "Find the next match (F3)", MenuAction::EditFindNext),
                MenuItem::new('G', "Go-to", "Jump to a cell address or name (Ctrl+G)", MenuAction::EditGoTo),
            ]),
        MenuItem::new('C', "Column", "Insert, delete and size columns", MenuAction::None)
            .with_submenus(vec![
                MenuItem::new('I', "Insert", "Insert a column before the cursor", MenuAction::InsertColumn),
                MenuItem::new('D', "Delete", "Delete the cursor's column", MenuAction::DeleteColumn),
                MenuItem::new('W', "Width", "Set the column width (Ctrl+Shift+Left/Right)", MenuAction::ColumnWidth),
                MenuItem::new('F', "Fit", "Fit the column to its widest value", MenuAction::ColumnFit),
                MenuItem::new('C', "Collapse", "Squeeze the column down to a sliver (Ctrl+0)", MenuAction::ColumnCollapse),
                MenuItem::new('E', "Expand", "Restore a collapsed column (Ctrl+Shift+0)", MenuAction::ColumnExpand),
            ]),
        MenuItem::new('R', "Row", "Insert, delete and size rows", MenuAction::None)
            .with_submenus(vec![
                MenuItem::new('I', "Insert", "Insert a row above the cursor", MenuAction::InsertRow),
                MenuItem::new('D', "Delete", "Delete the cursor's row", MenuAction::DeleteRow),
                MenuItem::new('H', "Height", "Set the row height (Ctrl+Shift+Up/Down)", MenuAction::RowHeight),
                MenuItem::new('C', "Collapse", "Squeeze the row down to a sliver (Ctrl+9)", MenuAction::RowCollapse),
                MenuItem::new('E', "Expand", "Restore a collapsed row (Ctrl+Shift+9)", MenuAction::RowExpand),
            ]),
        MenuItem::new('D', "Data", "Fill and sort the selection", MenuAction::None)
            .with_submenus(vec![
                MenuItem::new('F', "Fill", "Copy the first row or column through the selection", MenuAction::None)
                    .with_submenus(vec![
                        MenuItem::new('D', "Down", "Fill down (Ctrl+D)", MenuAction::FillDown),
                        MenuItem::new('R', "Right", "Fill right (Ctrl+R)", MenuAction::FillRight),
                    ]),
                MenuItem::new('S', "Sort", "Sort the selected rows by the cursor's column", MenuAction::None)
                    .with_submenus(vec![
                        MenuItem::new('A', "Ascending", "Smallest first; empty cells last", MenuAction::SortAscending),
                        MenuItem::new('D', "Descending", "Largest first; empty cells last", MenuAction::SortDescending),
                    ]),
            ]),
        MenuItem::new('F', "File", "Disk operations and workbook persistence", MenuAction::None)
            .with_submenus(vec![
                MenuItem::new('S', "Save", "Save workbook changes to disk", MenuAction::FileSave),
                MenuItem::new('R', "Retrieve", "Reload workbook from disk", MenuAction::FileReload),
                MenuItem::new('I', "Inspect", "Inspect workbook structure and payloads", MenuAction::FileInspect),
            ]),
        MenuItem::new('B', "Basic", "Embedded Modern BASIC script operations", MenuAction::None)
            .with_submenus(vec![
                MenuItem::new('R', "Run", "Execute embedded BASIC procedure in current sheet", MenuAction::BasicRun),
            ]),
        MenuItem::new('T', "TOC", "Dynamic Table of Contents operations", MenuAction::None)
            .with_submenus(vec![
                MenuItem::new('J', "Jump", "Jump cursor to dynamic TOC position (maxX+1, maxY+1)", MenuAction::TOCJump),
            ]),
        MenuItem::new('Q', "Quit", "Exit Lotus 5-7-11 workspace", MenuAction::Quit),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_navigation_and_first_letter_shortcuts() {
        let mut menu = MenuState::new();
        assert!(!menu.is_active);

        // Press '/' to activate
        menu.open();
        assert!(menu.is_active);

        let (l1, l2) = menu.render_lines();
        assert!(l1.contains("[Worksheet]"));
        assert!(l2.contains("Global spreadsheet settings"));

        // Type 'F' (File menu) -> should jump to File and enter its submenu
        let action = menu.handle_key(Some('F'), false, false, false, false);
        assert_eq!(action, MenuAction::None);

        let (l1_file, _) = menu.render_lines();
        assert!(l1_file.contains("[Save]"));

        // Type 'S' (Save action) -> should trigger FileSave action and close menu
        let save_action = menu.handle_key(Some('S'), false, false, false, false);
        assert_eq!(save_action, MenuAction::FileSave);
        assert!(!menu.is_active);

        // Test /WO (Worksheet -> Orphans)
        menu.open();
        let action = menu.handle_key(Some('W'), false, false, false, false);
        assert_eq!(action, MenuAction::None);
        let orphan_action = menu.handle_key(Some('O'), false, false, false, false);
        assert_eq!(orphan_action, MenuAction::OrphanCheck);
        assert!(!menu.is_active);
    }

    #[test]
    fn test_menu_escape_key() {
        let mut menu = MenuState::new();
        menu.open();

        // Enter Worksheet submenu
        menu.handle_key(None, false, false, true, false);
        assert_eq!(menu.breadcrumbs.len(), 2);

        // Esc returns to top level
        menu.handle_key(None, false, false, false, true);
        assert_eq!(menu.breadcrumbs.len(), 1);

        // Esc at top level dismisses menu
        menu.handle_key(None, false, false, false, true);
        assert!(!menu.is_active);
    }

    #[test]
    fn default_menu_tree_is_valid() {
        assert_eq!(validate_menu(&default_lotus_menu()), Ok(()));
    }

    #[test]
    fn validate_menu_rejects_duplicate_hotkeys_on_one_level() {
        let tree = vec![
            MenuItem::new('F', "File", "", MenuAction::None).with_submenus(vec![
                MenuItem::new('S', "Save", "", MenuAction::FileSave),
                MenuItem::new('S', "Save As", "", MenuAction::FileSave),
            ]),
        ];
        let err = validate_menu(&tree).unwrap_err();
        assert!(err.contains("duplicate hotkey 'S'"), "{err}");
        assert!(err.contains("/F"), "{err}");
    }

    #[test]
    fn validate_menu_allows_same_hotkey_on_different_levels() {
        // /FR and /WR share 'R' because they live under different groups.
        let tree = vec![
            MenuItem::new('F', "File", "", MenuAction::None)
                .with_submenus(vec![MenuItem::new('R', "Retrieve", "", MenuAction::FileReload)]),
            MenuItem::new('W', "Worksheet", "", MenuAction::None)
                .with_submenus(vec![MenuItem::new('R', "Recalculate", "", MenuAction::RecalculateAll)]),
        ];
        assert_eq!(validate_menu(&tree), Ok(()));
    }

    #[test]
    fn validate_menu_rejects_dead_leaves_and_groups_with_actions() {
        let dead_leaf = vec![MenuItem::new('X', "Nothing", "", MenuAction::None)];
        assert!(validate_menu(&dead_leaf).unwrap_err().contains("neither an action nor submenus"));

        let busy_group = vec![MenuItem::new('G', "Group", "", MenuAction::Quit)
            .with_submenus(vec![MenuItem::new('Q', "Quit", "", MenuAction::Quit)])];
        assert!(validate_menu(&busy_group).unwrap_err().contains("group but also carries an action"));
    }
}
