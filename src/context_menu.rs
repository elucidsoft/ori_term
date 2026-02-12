//! Custom GPU-rendered context menus (cross-platform).
//!
//! Chrome-style popup menus drawn through the existing wgpu overlay pipeline.
//! Three menu types:
//! - **Tab menu**: right-click on a tab → Close / Duplicate / Move to New Window
//! - **Tab bar menu**: right-click on empty tab bar area → New Tab
//! - **Dropdown menu**: dropdown button → Settings + Color Scheme submenu

use crate::palette::BUILTIN_SCHEMES;
use crate::render::FontSet;

// ── Menu styling constants (Chrome-inspired, in logical pixels) ─────────────

/// Corner radius for the menu popup.
const MENU_RADIUS: f32 = 8.0;
/// Vertical padding inside the menu (top and bottom).
const MENU_PADDING_Y: f32 = 4.0;
/// Height of a normal menu item.
const ITEM_HEIGHT: f32 = 32.0;
/// Horizontal padding for item text.
const ITEM_PADDING_X: f32 = 12.0;
/// Corner radius for the hover highlight on items.
const ITEM_HOVER_RADIUS: f32 = 4.0;
/// Horizontal inset of the hover highlight from the menu edge.
const ITEM_HOVER_INSET: f32 = 4.0;
/// Height of a separator (including its own vertical margins).
const SEPARATOR_HEIGHT: f32 = 9.0;
/// Thickness of the separator line.
const SEPARATOR_THICKNESS: f32 = 1.0;
/// Horizontal margin of the separator line from the menu edge.
const SEPARATOR_MARGIN_X: f32 = 12.0;
/// Minimum menu width.
const MENU_MIN_WIDTH: f32 = 180.0;
/// Extra width added to the right of the widest item text.
const MENU_EXTRA_WIDTH: f32 = 48.0;
/// Logical size of the checkmark icon.
pub const CHECKMARK_ICON_SIZE: f32 = 10.0;
/// Gap between checkmark icon and label text.
pub const CHECKMARK_GAP: f32 = 4.0;

/// Action resolved from a menu selection.
#[derive(Debug, Clone)]
pub enum ContextAction {
    CloseTab(usize),
    DuplicateTab(usize),
    MoveTabToNewWindow(usize),
    NewTab,
    OpenSettings,
    SelectScheme(String),
}

/// A single entry in a context menu.
#[derive(Debug, Clone)]
pub enum MenuEntry {
    /// A clickable text item.
    Item {
        label: String,
        action: ContextAction,
    },
    /// A checkable item (displays a checkmark when active).
    Check {
        label: String,
        checked: bool,
        action: ContextAction,
    },
    /// A visual separator line.
    Separator,
}

impl MenuEntry {
    fn label(&self) -> Option<&str> {
        match self {
            Self::Item { label, .. } | Self::Check { label, .. } => Some(label),
            Self::Separator => None,
        }
    }

    pub fn height(&self) -> f32 {
        match self {
            Self::Item { .. } | Self::Check { .. } => ITEM_HEIGHT,
            Self::Separator => SEPARATOR_HEIGHT,
        }
    }

    fn is_clickable(&self) -> bool {
        !matches!(self, Self::Separator)
    }
}

/// State for an open context menu overlay.
pub struct MenuOverlay {
    /// Menu entries.
    pub entries: Vec<MenuEntry>,
    /// Position of the menu top-left corner in physical pixels.
    pub position: (f32, f32),
    /// Index of the currently hovered item (if any).
    pub hovered: Option<usize>,
    /// Computed menu width in physical pixels (set during rendering).
    pub width: f32,
    /// Computed menu height in physical pixels (set during rendering).
    pub height: f32,
    /// Scale factor at the time the menu was created.
    pub scale: f32,
}

impl MenuOverlay {
    /// Compute layout dimensions. Call once after creating the overlay.
    pub fn layout(&mut self, glyphs: &mut FontSet) {
        let s = self.scale;
        // Checkmark rendered as a vector icon — use fixed icon size + gap
        let check_prefix_w = CHECKMARK_ICON_SIZE * s + CHECKMARK_GAP * s;
        let has_checks = self
            .entries
            .iter()
            .any(|e| matches!(e, MenuEntry::Check { .. }));

        let max_label_width = self
            .entries
            .iter()
            .filter_map(|e| {
                let label = e.label()?;
                let text_w = glyphs.text_advance(label);
                let extra = if has_checks { check_prefix_w } else { 0.0 };
                Some(text_w + extra)
            })
            .fold(0.0_f32, f32::max);

        self.width = (max_label_width + (ITEM_PADDING_X * 2.0 + MENU_EXTRA_WIDTH) * s)
            .max(MENU_MIN_WIDTH * s);
        self.height =
            MENU_PADDING_Y * 2.0 * s + self.entries.iter().map(|e| e.height() * s).sum::<f32>();
    }

    /// Hit-test a physical pixel position. Returns the entry index if the
    /// cursor is over a clickable item, or `None`.
    pub fn hit_test(&self, px: f32, py: f32) -> Option<usize> {
        let s = self.scale;
        let (mx, my) = self.position;
        if px < mx || px > mx + self.width || py < my || py > my + self.height {
            return None;
        }
        let mut y = my + MENU_PADDING_Y * s;
        for (i, entry) in self.entries.iter().enumerate() {
            let h = entry.height() * s;
            if py >= y && py < y + h && entry.is_clickable() {
                return Some(i);
            }
            y += h;
        }
        None
    }

    /// Returns the action for the currently hovered item (consuming it).
    pub fn activate_hovered(&self) -> Option<ContextAction> {
        let idx = self.hovered?;
        let entry = self.entries.get(idx)?;
        match entry {
            MenuEntry::Item { action, .. } | MenuEntry::Check { action, .. } => {
                Some(action.clone())
            }
            MenuEntry::Separator => None,
        }
    }

    /// Returns true if the given physical pixel position is inside the menu.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        let (mx, my) = self.position;
        px >= mx && px <= mx + self.width && py >= my && py <= my + self.height
    }

    /// Rendering constants (scaled).
    pub fn menu_radius(&self) -> f32 {
        MENU_RADIUS * self.scale
    }
    pub fn menu_padding_y(&self) -> f32 {
        MENU_PADDING_Y * self.scale
    }
    pub fn item_padding_x(&self) -> f32 {
        ITEM_PADDING_X * self.scale
    }
    pub fn item_hover_radius(&self) -> f32 {
        ITEM_HOVER_RADIUS * self.scale
    }
    pub fn item_hover_inset(&self) -> f32 {
        ITEM_HOVER_INSET * self.scale
    }
    pub fn separator_thickness(&self) -> f32 {
        SEPARATOR_THICKNESS * self.scale
    }
    pub fn separator_margin_x(&self) -> f32 {
        SEPARATOR_MARGIN_X * self.scale
    }
}

// ── Menu builders ───────────────────────────────────────────────────────────

/// Build the tab right-click menu.
pub fn build_tab_menu(position: (f32, f32), tab_index: usize, scale: f32) -> MenuOverlay {
    MenuOverlay {
        entries: vec![
            MenuEntry::Item {
                label: "Close Tab".into(),
                action: ContextAction::CloseTab(tab_index),
            },
            MenuEntry::Item {
                label: "Duplicate Tab".into(),
                action: ContextAction::DuplicateTab(tab_index),
            },
            MenuEntry::Item {
                label: "Move to New Window".into(),
                action: ContextAction::MoveTabToNewWindow(tab_index),
            },
        ],
        position,
        hovered: None,
        width: 0.0,
        height: 0.0,
        scale,
    }
}

/// Build the tab bar right-click menu (empty area).
pub fn build_tab_bar_menu(position: (f32, f32), scale: f32) -> MenuOverlay {
    MenuOverlay {
        entries: vec![MenuEntry::Item {
            label: "New Tab".into(),
            action: ContextAction::NewTab,
        }],
        position,
        hovered: None,
        width: 0.0,
        height: 0.0,
        scale,
    }
}

/// Build the dropdown menu (settings + color scheme submenu).
pub fn build_dropdown_menu(position: (f32, f32), active_scheme: &str, scale: f32) -> MenuOverlay {
    let mut entries = vec![
        MenuEntry::Item {
            label: "Settings".into(),
            action: ContextAction::OpenSettings,
        },
        MenuEntry::Separator,
    ];

    for scheme in BUILTIN_SCHEMES {
        entries.push(MenuEntry::Check {
            label: scheme.name.to_owned(),
            checked: scheme.name == active_scheme,
            action: ContextAction::SelectScheme(scheme.name.to_owned()),
        });
    }

    MenuOverlay {
        entries,
        position,
        hovered: None,
        width: 0.0,
        height: 0.0,
        scale,
    }
}
