use crate::render::{GlyphCache, render_text};
use crate::tab::TabId;

// Tab bar constants
pub const TAB_BAR_HEIGHT: usize = 30;
const TAB_MIN_WIDTH: usize = 80;
const TAB_MAX_WIDTH: usize = 200;
const TAB_PADDING: usize = 8;
const CLOSE_BUTTON_WIDTH: usize = 20;
const NEW_TAB_BUTTON_WIDTH: usize = 30;

// Catppuccin Mocha colors
const TAB_BAR_BG: u32 = 0x00181825;       // mantle
const TAB_ACTIVE_BG: u32 = 0x001e1e2e;    // base (matches terminal BG)
const TAB_INACTIVE_BG: u32 = 0x00313244;  // surface0
const TAB_HOVER_BG: u32 = 0x00363a4f;     // slightly lighter
const TAB_TEXT_FG: u32 = 0x00cdd6f4;       // text
const TAB_INACTIVE_TEXT: u32 = 0x00a6adc8; // subtext0
const TAB_BORDER: u32 = 0x00585b70;        // overlay0
const CLOSE_HOVER_BG: u32 = 0x00f38ba8;   // red (on hover)
const CLOSE_FG: u32 = 0x00a6adc8;         // subtext0

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabBarHit {
    Tab(usize),
    CloseTab(usize),
    NewTab,
    None,
}

pub struct TabBarLayout {
    pub tab_width: usize,
    pub tab_count: usize,
    pub bar_width: usize,
}

impl TabBarLayout {
    pub fn compute(tab_count: usize, bar_width: usize) -> Self {
        let available = bar_width.saturating_sub(NEW_TAB_BUTTON_WIDTH);
        let tab_width = if tab_count == 0 {
            TAB_MIN_WIDTH
        } else {
            (available / tab_count).clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH)
        };
        Self {
            tab_width,
            tab_count,
            bar_width,
        }
    }

    pub fn hit_test(&self, x: usize, y: usize) -> TabBarHit {
        if y >= TAB_BAR_HEIGHT {
            return TabBarHit::None;
        }

        // Check new tab button (at the end of all tabs)
        let tabs_end = self.tab_count * self.tab_width;
        if x >= tabs_end && x < tabs_end + NEW_TAB_BUTTON_WIDTH {
            return TabBarHit::NewTab;
        }

        // Check which tab
        if x < tabs_end {
            let tab_idx = x / self.tab_width;
            if tab_idx < self.tab_count {
                // Check close button (rightmost CLOSE_BUTTON_WIDTH pixels of tab)
                let tab_right = (tab_idx + 1) * self.tab_width;
                if x >= tab_right.saturating_sub(CLOSE_BUTTON_WIDTH) {
                    return TabBarHit::CloseTab(tab_idx);
                }
                return TabBarHit::Tab(tab_idx);
            }
        }

        TabBarHit::None
    }
}

pub fn render_tab_bar(
    glyphs: &mut GlyphCache,
    buffer: &mut [u32],
    buf_w: usize,
    _buf_h: usize,
    tabs: &[(TabId, String)],
    active_idx: usize,
    hover_hit: TabBarHit,
) {
    // Fill tab bar background
    for y in 0..TAB_BAR_HEIGHT {
        for x in 0..buf_w {
            buffer[y * buf_w + x] = TAB_BAR_BG;
        }
    }

    let layout = TabBarLayout::compute(tabs.len(), buf_w);
    let tab_w = layout.tab_width;

    for (i, (_id, title)) in tabs.iter().enumerate() {
        let x0 = i * tab_w;
        let is_active = i == active_idx;
        let is_hovered = hover_hit == TabBarHit::Tab(i);

        let bg = if is_active {
            TAB_ACTIVE_BG
        } else if is_hovered {
            TAB_HOVER_BG
        } else {
            TAB_INACTIVE_BG
        };

        let text_fg = if is_active {
            TAB_TEXT_FG
        } else {
            TAB_INACTIVE_TEXT
        };

        // Draw tab background
        for y in 2..TAB_BAR_HEIGHT {
            for x in x0..(x0 + tab_w).min(buf_w) {
                buffer[y * buf_w + x] = bg;
            }
        }

        // Draw right border
        let border_x = x0 + tab_w - 1;
        if border_x < buf_w {
            for y in 4..TAB_BAR_HEIGHT - 2 {
                buffer[y * buf_w + border_x] = TAB_BORDER;
            }
        }

        // Active tab bottom highlight (no border at bottom — blends with terminal)
        if is_active {
            let bottom = TAB_BAR_HEIGHT - 1;
            for x in x0..(x0 + tab_w).min(buf_w) {
                buffer[bottom * buf_w + x] = TAB_ACTIVE_BG;
            }
        }

        // Tab title — truncate to fit
        let max_text_chars = (tab_w - TAB_PADDING * 2 - CLOSE_BUTTON_WIDTH) / glyphs.cell_width;
        let display_title: String = if title.len() > max_text_chars {
            let mut t: String = title.chars().take(max_text_chars.saturating_sub(1)).collect();
            t.push('\u{2026}'); // ellipsis
            t
        } else {
            title.clone()
        };

        let text_y = (TAB_BAR_HEIGHT - glyphs.cell_height) / 2;
        render_text(
            glyphs,
            &display_title,
            text_fg,
            buffer,
            buf_w,
            TAB_BAR_HEIGHT,
            x0 + TAB_PADDING,
            text_y,
        );

        // Close button "x"
        let close_x = x0 + tab_w - CLOSE_BUTTON_WIDTH;
        let close_hovered = hover_hit == TabBarHit::CloseTab(i);
        if close_hovered {
            for y in 6..TAB_BAR_HEIGHT - 6 {
                for x in close_x..(close_x + CLOSE_BUTTON_WIDTH).min(buf_w) {
                    buffer[y * buf_w + x] = CLOSE_HOVER_BG;
                }
            }
        }
        let close_fg = if close_hovered { TAB_TEXT_FG } else { CLOSE_FG };
        let close_text_y = (TAB_BAR_HEIGHT - glyphs.cell_height) / 2;
        let close_text_x = close_x + (CLOSE_BUTTON_WIDTH - glyphs.cell_width) / 2;
        render_text(
            glyphs,
            "\u{00D7}", // multiplication sign (×) as close icon
            close_fg,
            buffer,
            buf_w,
            TAB_BAR_HEIGHT,
            close_text_x,
            close_text_y,
        );
    }

    // New tab "+" button
    let plus_x = tabs.len() * tab_w;
    if plus_x + NEW_TAB_BUTTON_WIDTH <= buf_w {
        let plus_hovered = hover_hit == TabBarHit::NewTab;
        let plus_bg = if plus_hovered { TAB_HOVER_BG } else { TAB_BAR_BG };

        for y in 2..TAB_BAR_HEIGHT {
            for x in plus_x..(plus_x + NEW_TAB_BUTTON_WIDTH).min(buf_w) {
                buffer[y * buf_w + x] = plus_bg;
            }
        }

        let plus_text_y = (TAB_BAR_HEIGHT - glyphs.cell_height) / 2;
        let plus_text_x = plus_x + (NEW_TAB_BUTTON_WIDTH - glyphs.cell_width) / 2;
        render_text(
            glyphs,
            "+",
            TAB_TEXT_FG,
            buffer,
            buf_w,
            TAB_BAR_HEIGHT,
            plus_text_x,
            plus_text_y,
        );
    }

    // Bottom border of tab bar
    let bottom_y = TAB_BAR_HEIGHT - 1;
    for x in 0..buf_w {
        // Don't overwrite active tab area
        if buffer[bottom_y * buf_w + x] != TAB_ACTIVE_BG {
            buffer[bottom_y * buf_w + x] = TAB_BORDER;
        }
    }
}
