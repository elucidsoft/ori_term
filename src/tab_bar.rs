use crate::render::{FontSet, render_text};
use crate::tab::TabId;

// Tab bar constants
pub const TAB_BAR_HEIGHT: usize = 46;
pub const TAB_LEFT_MARGIN: usize = 16;     // space between window edge and first tab
const TAB_TOP_MARGIN: usize = 8;       // space between window edge and tab tops
const TAB_BOTTOM_MARGIN: usize = 0;    // tabs touch the grid area
const TAB_MIN_WIDTH: usize = 80;
const TAB_MAX_WIDTH: usize = 200;
const TAB_PADDING: usize = 8;
const CLOSE_BUTTON_WIDTH: usize = 24;
const CLOSE_BUTTON_RIGHT_PAD: usize = 8;  // padding from right edge of tab
pub const NEW_TAB_BUTTON_WIDTH: usize = 38;
pub const DROPDOWN_BUTTON_WIDTH: usize = 30;

// Window control button constants (Windows 10/11 proportions)
const CONTROL_BUTTON_WIDTH: usize = 58;
const CONTROLS_ZONE_WIDTH: usize = CONTROL_BUTTON_WIDTH * 3; // 174px for 3 buttons

// Window control icon size (10x10 pixel icons, centered in buttons)
const ICON_SIZE: usize = 10;

// Catppuccin Mocha colors
const TAB_BAR_BG: u32 = 0x00181825;       // mantle
const TAB_ACTIVE_BG: u32 = 0x00000000;    // black (matches terminal BG)
const TAB_INACTIVE_BG: u32 = 0x00313244;  // surface0
const TAB_HOVER_BG: u32 = 0x00363a4f;     // slightly lighter
const TAB_TEXT_FG: u32 = 0x00cdd6f4;       // text
const TAB_INACTIVE_TEXT: u32 = 0x00a6adc8; // subtext0
const TAB_BORDER: u32 = 0x00282838;        // subtle border, close to mantle
const _CLOSE_HOVER_BG: u32 = 0x00333345;   // dark gray (on hover, reserved for future use)
const CLOSE_FG: u32 = 0x00a6adc8;         // subtext0

// Window control colors (Windows 10/11 style)
const CONTROL_HOVER_BG: u32 = 0x00333345;       // subtle lighten for min/max hover
const CONTROL_CLOSE_HOVER_BG: u32 = 0x00e81123;  // Windows red
const CONTROL_FG: u32 = 0x00cdd6f4;              // text color (bright for dark bg)
const CONTROL_CLOSE_HOVER_FG: u32 = 0x00ffffff;   // white on red

// Window border
pub const WINDOW_BORDER_COLOR: u32 = 0x00585b70;  // overlay0 accent border
pub const WINDOW_BORDER_WIDTH: usize = 1;

// Grid inset from window edges
pub const GRID_PADDING_LEFT: usize = 6;
pub const GRID_PADDING_BOTTOM: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabBarHit {
    Tab(usize),
    CloseTab(usize),
    NewTab,
    DropdownButton,
    Minimize,
    Maximize,
    CloseWindow,
    DragArea,
    None,
}

pub struct TabBarLayout {
    pub tab_width: usize,
    pub tab_count: usize,
    pub bar_width: usize,
}

impl TabBarLayout {
    pub fn compute(tab_count: usize, bar_width: usize) -> Self {
        // Reserve space for left margin, new-tab button, dropdown, and window controls
        let available = bar_width
            .saturating_sub(TAB_LEFT_MARGIN)
            .saturating_sub(NEW_TAB_BUTTON_WIDTH)
            .saturating_sub(DROPDOWN_BUTTON_WIDTH)
            .saturating_sub(CONTROLS_ZONE_WIDTH);
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

        // Check window controls zone (rightmost CONTROLS_ZONE_WIDTH pixels)
        let controls_start = self.bar_width.saturating_sub(CONTROLS_ZONE_WIDTH);
        if x >= controls_start {
            let offset = x - controls_start;
            let button_idx = offset / CONTROL_BUTTON_WIDTH;
            return match button_idx {
                0 => TabBarHit::Minimize,
                1 => TabBarHit::Maximize,
                _ => TabBarHit::CloseWindow,
            };
        }

        // Tabs start after the left margin
        let tab_x = x.saturating_sub(TAB_LEFT_MARGIN);
        let tabs_end = TAB_LEFT_MARGIN + self.tab_count * self.tab_width;

        // Check new tab button (at the end of all tabs)
        if x >= tabs_end && x < tabs_end + NEW_TAB_BUTTON_WIDTH {
            return TabBarHit::NewTab;
        }

        // Check dropdown button (right after new-tab button)
        let dropdown_x = tabs_end + NEW_TAB_BUTTON_WIDTH;
        if x >= dropdown_x && x < dropdown_x + DROPDOWN_BUTTON_WIDTH {
            return TabBarHit::DropdownButton;
        }

        // Check which tab
        if x >= TAB_LEFT_MARGIN && x < tabs_end {
            let tab_idx = tab_x / self.tab_width;
            if tab_idx < self.tab_count {
                // Check close button (inset from right edge of tab)
                let tab_right = (tab_idx + 1) * self.tab_width;
                let close_start = tab_right.saturating_sub(CLOSE_BUTTON_WIDTH + CLOSE_BUTTON_RIGHT_PAD);
                if tab_x >= close_start && tab_x < tab_right.saturating_sub(CLOSE_BUTTON_RIGHT_PAD) {
                    return TabBarHit::CloseTab(tab_idx);
                }
                return TabBarHit::Tab(tab_idx);
            }
        }

        // Empty space between new-tab button and controls = drag area
        TabBarHit::DragArea
    }
}

pub fn render_tab_bar(
    glyphs: &mut FontSet,
    buffer: &mut [u32],
    buf_w: usize,
    _buf_h: usize,
    tabs: &[(TabId, String)],
    active_idx: usize,
    hover_hit: TabBarHit,
    is_maximized: bool,
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
        let x0 = TAB_LEFT_MARGIN + i * tab_w;
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

        // Draw tab background with slightly rounded top corners
        let top = TAB_TOP_MARGIN;
        let bot = TAB_BAR_HEIGHT - TAB_BOTTOM_MARGIN;
        for y in top..bot {
            for x in x0..(x0 + tab_w).min(buf_w) {
                // Top-left corner cutoff (2px)
                if y == top && x < x0 + 2 {
                    continue;
                }
                // Top-right corner cutoff (2px)
                if y == top && x >= x0 + tab_w - 2 {
                    continue;
                }
                // Second row corner cutoff (1px)
                if y == top + 1 && x == x0 {
                    continue;
                }
                if y == top + 1 && x == x0 + tab_w - 1 {
                    continue;
                }
                buffer[y * buf_w + x] = bg;
            }
        }

        // Draw right border
        let border_x = x0 + tab_w - 1;
        if border_x < buf_w {
            for y in (top + 2)..bot.saturating_sub(2) {
                buffer[y * buf_w + border_x] = TAB_BORDER;
            }
        }

        // Active tab bottom highlight (blends tab into bar bg below)
        if is_active {
            for x in x0..(x0 + tab_w).min(buf_w) {
                buffer[(bot - 1) * buf_w + x] = TAB_ACTIVE_BG;
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

        let text_y = TAB_TOP_MARGIN + (TAB_BAR_HEIGHT - TAB_TOP_MARGIN - glyphs.cell_height) / 2;
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
        let close_x = x0 + tab_w - CLOSE_BUTTON_WIDTH - CLOSE_BUTTON_RIGHT_PAD;
        let close_hovered = hover_hit == TabBarHit::CloseTab(i);
        if close_hovered {
            let sq_y = TAB_TOP_MARGIN + (TAB_BAR_HEIGHT - TAB_TOP_MARGIN - CLOSE_BUTTON_WIDTH) / 2;
            for y in sq_y..(sq_y + CLOSE_BUTTON_WIDTH) {
                for x in close_x..(close_x + CLOSE_BUTTON_WIDTH).min(buf_w) {
                    let idx = y * buf_w + x;
                    buffer[idx] = blend(buffer[idx], 0x00ffffff, 40);
                }
            }
        }
        let close_fg = if close_hovered { TAB_TEXT_FG } else { CLOSE_FG };
        // Pixel-drawn X, 8x8, centered in the 16x16 square
        let x_size: usize = 8;
        let sq_y = TAB_TOP_MARGIN + (TAB_BAR_HEIGHT - TAB_TOP_MARGIN - CLOSE_BUTTON_WIDTH) / 2;
        let x_cx = close_x + (CLOSE_BUTTON_WIDTH - x_size) / 2;
        let x_cy = sq_y + (CLOSE_BUTTON_WIDTH - x_size) / 2;
        draw_x(buffer, buf_w, x_cx, x_cy, x_size, close_fg);
    }

    // New tab "+" button
    let plus_x = TAB_LEFT_MARGIN + tabs.len() * tab_w;
    if plus_x + NEW_TAB_BUTTON_WIDTH <= buf_w {
        let plus_hovered = hover_hit == TabBarHit::NewTab;
        let plus_bg = if plus_hovered { TAB_HOVER_BG } else { TAB_BAR_BG };

        let bot = TAB_BAR_HEIGHT - TAB_BOTTOM_MARGIN;
        for y in TAB_TOP_MARGIN..bot {
            for x in plus_x..(plus_x + NEW_TAB_BUTTON_WIDTH).min(buf_w) {
                buffer[y * buf_w + x] = plus_bg;
            }
        }

        // Pixel-drawn plus icon, 9x9, centered in the button
        let plus_size: usize = 11;
        let cx = plus_x + (NEW_TAB_BUTTON_WIDTH - plus_size) / 2;
        let cy = TAB_TOP_MARGIN + (TAB_BAR_HEIGHT - TAB_TOP_MARGIN - plus_size) / 2;
        draw_plus(buffer, buf_w, cx, cy, plus_size, TAB_TEXT_FG);
    }

    // Window control buttons (rightmost zone)
    let controls_start = buf_w.saturating_sub(CONTROLS_ZONE_WIDTH);
    render_window_controls(
        buffer,
        buf_w,
        controls_start,
        hover_hit,
        is_maximized,
    );

}

/// Draw a 1px border around the entire window (Windows 10 style accent border).
/// Call this after all other rendering so it's on top.
pub fn render_window_border(buffer: &mut [u32], buf_w: usize, buf_h: usize, is_maximized: bool) {
    if is_maximized {
        return; // No border when maximized — fills the screen edge-to-edge
    }

    let color = WINDOW_BORDER_COLOR;

    // Top edge
    for pixel in buffer.iter_mut().take(buf_w) {
        *pixel = color;
    }
    // Bottom edge
    if buf_h > 0 {
        let row = (buf_h - 1) * buf_w;
        for x in 0..buf_w {
            buffer[row + x] = color;
        }
    }
    // Left edge
    for y in 0..buf_h {
        buffer[y * buf_w] = color;
    }
    // Right edge
    if buf_w > 0 {
        for y in 0..buf_h {
            buffer[y * buf_w + buf_w - 1] = color;
        }
    }
}

fn render_window_controls(
    buffer: &mut [u32],
    buf_w: usize,
    controls_start: usize,
    hover_hit: TabBarHit,
    is_maximized: bool,
) {
    // --- Minimize button ---
    {
        let btn_x = controls_start;
        let hovered = hover_hit == TabBarHit::Minimize;
        if hovered {
            fill_rect(buffer, buf_w, btn_x, 0, CONTROL_BUTTON_WIDTH, TAB_BAR_HEIGHT, CONTROL_HOVER_BG);
        }
        let fg = CONTROL_FG;
        // Horizontal line: 10px wide, 1px tall, centered
        let cx = btn_x + (CONTROL_BUTTON_WIDTH - ICON_SIZE) / 2;
        let cy = TAB_BAR_HEIGHT / 2;
        draw_hline(buffer, buf_w, cx, cy, ICON_SIZE, fg);
    }

    // --- Maximize / Restore button ---
    {
        let btn_x = controls_start + CONTROL_BUTTON_WIDTH;
        let hovered = hover_hit == TabBarHit::Maximize;
        if hovered {
            fill_rect(buffer, buf_w, btn_x, 0, CONTROL_BUTTON_WIDTH, TAB_BAR_HEIGHT, CONTROL_HOVER_BG);
        }
        let fg = CONTROL_FG;
        let cx = btn_x + (CONTROL_BUTTON_WIDTH - ICON_SIZE) / 2;
        let cy = (TAB_BAR_HEIGHT - ICON_SIZE) / 2;

        if is_maximized {
            // Restore icon: two overlapping rectangles
            // Back rect (offset +2, -2 from front)
            let back_size = ICON_SIZE - 2;
            draw_rect(buffer, buf_w, cx + 2, cy, back_size, back_size, fg);
            // Front rect (offset 0, +2 from back)
            draw_rect(buffer, buf_w, cx, cy + 2, back_size, back_size, fg);
            // Fill the front rect interior with the button bg to cover the back rect lines
            let inner_bg = if hovered { CONTROL_HOVER_BG } else { TAB_BAR_BG };
            fill_rect(buffer, buf_w, cx + 1, cy + 3, back_size - 2, back_size - 2, inner_bg);
        } else {
            // Maximize icon: single 10x10 rectangle outline
            draw_rect(buffer, buf_w, cx, cy, ICON_SIZE, ICON_SIZE, fg);
        }
    }

    // --- Close button ---
    {
        let btn_x = controls_start + CONTROL_BUTTON_WIDTH * 2;
        let hovered = hover_hit == TabBarHit::CloseWindow;
        if hovered {
            fill_rect(buffer, buf_w, btn_x, 0, CONTROL_BUTTON_WIDTH, TAB_BAR_HEIGHT, CONTROL_CLOSE_HOVER_BG);
        }
        let fg = if hovered { CONTROL_CLOSE_HOVER_FG } else { CONTROL_FG };
        let cx = btn_x + (CONTROL_BUTTON_WIDTH - ICON_SIZE) / 2;
        let cy = (TAB_BAR_HEIGHT - ICON_SIZE) / 2;
        // X shape: two diagonal lines
        draw_x(buffer, buf_w, cx, cy, ICON_SIZE, fg);
    }
}

// --- Pixel drawing helpers ---

/// Alpha-blend `fg` over `bg` with 0–255 alpha (0=fully bg, 255=fully fg).
fn blend(bg: u32, fg: u32, alpha: u32) -> u32 {
    let inv = 255 - alpha;
    let r = ((fg >> 16 & 0xFF) * alpha + (bg >> 16 & 0xFF) * inv) / 255;
    let g = ((fg >> 8 & 0xFF) * alpha + (bg >> 8 & 0xFF) * inv) / 255;
    let b = ((fg & 0xFF) * alpha + (bg & 0xFF) * inv) / 255;
    (r << 16) | (g << 8) | b
}

fn set_pixel(buffer: &mut [u32], buf_w: usize, x: usize, y: usize, color: u32) {
    if x < buf_w && y * buf_w + x < buffer.len() {
        buffer[y * buf_w + x] = color;
    }
}

fn fill_rect(buffer: &mut [u32], buf_w: usize, x: usize, y: usize, w: usize, h: usize, color: u32) {
    for dy in 0..h {
        for dx in 0..w {
            set_pixel(buffer, buf_w, x + dx, y + dy, color);
        }
    }
}

fn draw_hline(buffer: &mut [u32], buf_w: usize, x: usize, y: usize, len: usize, color: u32) {
    for dx in 0..len {
        set_pixel(buffer, buf_w, x + dx, y, color);
    }
}

fn draw_rect(buffer: &mut [u32], buf_w: usize, x: usize, y: usize, w: usize, h: usize, color: u32) {
    // Top edge
    for dx in 0..w {
        set_pixel(buffer, buf_w, x + dx, y, color);
    }
    // Bottom edge
    for dx in 0..w {
        set_pixel(buffer, buf_w, x + dx, y + h - 1, color);
    }
    // Left edge
    for dy in 0..h {
        set_pixel(buffer, buf_w, x, y + dy, color);
    }
    // Right edge
    for dy in 0..h {
        set_pixel(buffer, buf_w, x + w - 1, y + dy, color);
    }
}

fn draw_plus(buffer: &mut [u32], buf_w: usize, x: usize, y: usize, size: usize, color: u32) {
    let mid = size / 2;
    // Horizontal bar
    draw_hline(buffer, buf_w, x, y + mid, size, color);
    // Vertical bar
    for i in 0..size {
        set_pixel(buffer, buf_w, x + mid, y + i, color);
    }
}

fn draw_x(buffer: &mut [u32], buf_w: usize, x: usize, y: usize, size: usize, color: u32) {
    for i in 0..size {
        // Main diagonal (\)
        set_pixel(buffer, buf_w, x + i, y + i, color);
        // Anti-diagonal (/)
        set_pixel(buffer, buf_w, x + size - 1 - i, y + i, color);
    }
}
