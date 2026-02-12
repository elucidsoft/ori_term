use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::render::{FontSet, render_text};
use crate::tab::TabId;

// Tab bar constants
pub const TAB_BAR_HEIGHT: usize = 46;
pub const TAB_LEFT_MARGIN: usize = 16;     // space between window edge and first tab
const TAB_TOP_MARGIN: usize = 8;       // space between window edge and tab tops
const TAB_BOTTOM_MARGIN: usize = 0;    // tabs touch the grid area
const TAB_MIN_WIDTH: usize = 80;
const TAB_MAX_WIDTH: usize = 260;
const TAB_PADDING: usize = 8;
const CLOSE_BUTTON_WIDTH: usize = 24;
const CLOSE_BUTTON_RIGHT_PAD: usize = 8;  // padding from right edge of tab
pub const NEW_TAB_BUTTON_WIDTH: usize = 38;
pub const DROPDOWN_BUTTON_WIDTH: usize = 30;

// Window control button constants — Windows 10/11 proportions
#[cfg(target_os = "windows")]
const CONTROL_BUTTON_WIDTH: usize = 58;
#[cfg(target_os = "windows")]
pub const CONTROLS_ZONE_WIDTH: usize = CONTROL_BUTTON_WIDTH * 3; // 174px for 3 buttons
#[cfg(target_os = "windows")]
const ICON_SIZE: usize = 10;

// Window control button constants — Linux (GNOME-style circles)
#[cfg(not(target_os = "windows"))]
const CONTROL_BUTTON_DIAMETER: usize = 24;
#[cfg(not(target_os = "windows"))]
const CONTROL_BUTTON_SPACING: usize = 8;
#[cfg(not(target_os = "windows"))]
const CONTROL_BUTTON_MARGIN: usize = 12;
#[cfg(not(target_os = "windows"))]
pub const CONTROLS_ZONE_WIDTH: usize =
    CONTROL_BUTTON_MARGIN + 3 * CONTROL_BUTTON_DIAMETER + 2 * CONTROL_BUTTON_SPACING + CONTROL_BUTTON_MARGIN;
// = 12 + 60 + 16 + 12 = 100px
#[cfg(not(target_os = "windows"))]
const ICON_SIZE: usize = 8;
#[cfg(not(target_os = "windows"))]
const CONTROL_CIRCLE_ALPHA: u32 = 77; // 0–255, semi-transparent (~30%)

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

// Window control colors — Windows 10/11 style
#[cfg(target_os = "windows")]
const CONTROL_HOVER_BG: u32 = 0x00333345;       // subtle lighten for min/max hover
#[cfg(target_os = "windows")]
const CONTROL_CLOSE_HOVER_BG: u32 = 0x00e81123;  // Windows red
#[cfg(target_os = "windows")]
const CONTROL_FG: u32 = 0x00cdd6f4;              // text color (bright for dark bg)
#[cfg(target_os = "windows")]
const CONTROL_CLOSE_HOVER_FG: u32 = 0x00ffffff;   // white on red

// Window control colors — Linux (GNOME-style)
#[cfg(not(target_os = "windows"))]
const CONTROL_CIRCLE_BG: u32 = 0x00404050;        // subtle gray circle
#[cfg(not(target_os = "windows"))]
const CONTROL_HOVER_BG: u32 = 0x00505060;         // lighter on hover
#[cfg(not(target_os = "windows"))]
const CONTROL_CLOSE_HOVER_BG: u32 = 0x00c01c28;   // GNOME red
#[cfg(not(target_os = "windows"))]
const CONTROL_FG: u32 = 0x00cdd6f4;               // icon color
#[cfg(not(target_os = "windows"))]
const CONTROL_CLOSE_HOVER_FG: u32 = 0x00ffffff;    // white on red

// Window border (Windows only — on Linux the WM draws its own border/shadow)
#[cfg(target_os = "windows")]
pub const WINDOW_BORDER_COLOR: u32 = 0x00585b70;  // overlay0 accent border
#[cfg(target_os = "windows")]
pub const WINDOW_BORDER_WIDTH: usize = 1;

// Grid inset from window edges
pub const GRID_PADDING_LEFT: usize = 6;
pub const GRID_PADDING_TOP: usize = 10;
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
    pub fn compute(tab_count: usize, bar_width: usize, scale: f64) -> Self {
        let s = |v: usize| -> usize { (v as f64 * scale).round() as usize };
        // Reserve space for left margin, new-tab button, dropdown, and window controls
        let available = bar_width
            .saturating_sub(s(TAB_LEFT_MARGIN))
            .saturating_sub(s(NEW_TAB_BUTTON_WIDTH))
            .saturating_sub(s(DROPDOWN_BUTTON_WIDTH))
            .saturating_sub(s(CONTROLS_ZONE_WIDTH));
        let tab_width = if tab_count == 0 {
            s(TAB_MIN_WIDTH)
        } else {
            (available / tab_count).clamp(s(TAB_MIN_WIDTH), s(TAB_MAX_WIDTH))
        };
        Self {
            tab_width,
            tab_count,
            bar_width,
        }
    }

    pub fn hit_test(&self, x: usize, y: usize, scale: f64) -> TabBarHit {
        let s = |v: usize| -> usize { (v as f64 * scale).round() as usize };

        if y >= s(TAB_BAR_HEIGHT) {
            return TabBarHit::None;
        }

        // Check window controls zone (rightmost CONTROLS_ZONE_WIDTH pixels)
        let controls_start = self.bar_width.saturating_sub(s(CONTROLS_ZONE_WIDTH));
        if x >= controls_start {
            #[cfg(target_os = "windows")]
            {
                let offset = x - controls_start;
                let button_idx = offset / s(CONTROL_BUTTON_WIDTH);
                return match button_idx {
                    0 => TabBarHit::Minimize,
                    1 => TabBarHit::Maximize,
                    _ => TabBarHit::CloseWindow,
                };
            }
            #[cfg(not(target_os = "windows"))]
            {
                // GNOME-style: circular hit-test for each button
                let cy = s(TAB_BAR_HEIGHT) / 2;
                let r = s(CONTROL_BUTTON_DIAMETER) / 2;
                let buttons = [
                    (controls_start + s(CONTROL_BUTTON_MARGIN) + r, TabBarHit::Minimize),
                    (controls_start + s(CONTROL_BUTTON_MARGIN) + s(CONTROL_BUTTON_DIAMETER) + s(CONTROL_BUTTON_SPACING) + r, TabBarHit::Maximize),
                    (controls_start + s(CONTROL_BUTTON_MARGIN) + 2 * (s(CONTROL_BUTTON_DIAMETER) + s(CONTROL_BUTTON_SPACING)) + r, TabBarHit::CloseWindow),
                ];
                for &(cx, hit) in &buttons {
                    let dx = x as isize - cx as isize;
                    let dy = y as isize - cy as isize;
                    if dx * dx + dy * dy <= (r as isize) * (r as isize) {
                        return hit;
                    }
                }
                // Click in the controls zone but not on a circle = drag area
                return TabBarHit::DragArea;
            }
        }

        // Tabs start after the left margin
        let left_margin = s(TAB_LEFT_MARGIN);
        let tab_x = x.saturating_sub(left_margin);
        let tabs_end = left_margin + self.tab_count * self.tab_width;

        // Check new tab button (at the end of all tabs)
        let new_tab_w = s(NEW_TAB_BUTTON_WIDTH);
        if x >= tabs_end && x < tabs_end + new_tab_w {
            return TabBarHit::NewTab;
        }

        // Check dropdown button (right after new-tab button)
        let dropdown_w = s(DROPDOWN_BUTTON_WIDTH);
        let dropdown_x = tabs_end + new_tab_w;
        if x >= dropdown_x && x < dropdown_x + dropdown_w {
            return TabBarHit::DropdownButton;
        }

        // Check which tab
        if x >= left_margin && x < tabs_end {
            let tab_idx = tab_x / self.tab_width;
            if tab_idx < self.tab_count {
                // Check close button (inset from right edge of tab)
                let tab_right = (tab_idx + 1) * self.tab_width;
                let close_start = tab_right.saturating_sub(s(CLOSE_BUTTON_WIDTH) + s(CLOSE_BUTTON_RIGHT_PAD));
                if tab_x >= close_start && tab_x < tab_right.saturating_sub(s(CLOSE_BUTTON_RIGHT_PAD)) {
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

    let layout = TabBarLayout::compute(tabs.len(), buf_w, 1.0);
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

        // Tab title — truncate to fit (width-aware for CJK/emoji)
        let max_text_cells = (tab_w - TAB_PADDING * 2 - CLOSE_BUTTON_WIDTH) / glyphs.cell_width;
        let display_title: String = if UnicodeWidthStr::width(title.as_str()) > max_text_cells {
            truncate_to_width(title, max_text_cells)
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
/// On Linux, the WM draws its own border/shadow — this is a no-op.
#[cfg(target_os = "windows")]
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

#[cfg(not(target_os = "windows"))]
pub fn render_window_border(_buffer: &mut [u32], _buf_w: usize, _buf_h: usize, _is_maximized: bool) {
    // On Linux, the WM draws its own border/shadow.
}

#[cfg(target_os = "windows")]
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
            let back_size = ICON_SIZE - 2;
            draw_rect(buffer, buf_w, cx + 2, cy, back_size, back_size, fg);
            draw_rect(buffer, buf_w, cx, cy + 2, back_size, back_size, fg);
            let inner_bg = if hovered { CONTROL_HOVER_BG } else { TAB_BAR_BG };
            fill_rect(buffer, buf_w, cx + 1, cy + 3, back_size - 2, back_size - 2, inner_bg);
        } else {
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
        draw_x(buffer, buf_w, cx, cy, ICON_SIZE, fg);
    }
}

#[cfg(not(target_os = "windows"))]
fn render_window_controls(
    buffer: &mut [u32],
    buf_w: usize,
    controls_start: usize,
    hover_hit: TabBarHit,
    is_maximized: bool,
) {
    let cy = TAB_BAR_HEIGHT / 2;
    let r = CONTROL_BUTTON_DIAMETER / 2;

    // Button center X positions (minimize, maximize, close — left to right)
    let btn_centers = [
        controls_start + CONTROL_BUTTON_MARGIN + r,
        controls_start + CONTROL_BUTTON_MARGIN + CONTROL_BUTTON_DIAMETER + CONTROL_BUTTON_SPACING + r,
        controls_start + CONTROL_BUTTON_MARGIN + 2 * (CONTROL_BUTTON_DIAMETER + CONTROL_BUTTON_SPACING) + r,
    ];
    let hits = [TabBarHit::Minimize, TabBarHit::Maximize, TabBarHit::CloseWindow];

    for (i, &bcx) in btn_centers.iter().enumerate() {
        let hovered = hover_hit == hits[i];
        let is_close = i == 2;

        // Circle background
        let circle_bg = if hovered && is_close {
            CONTROL_CLOSE_HOVER_BG
        } else if hovered {
            CONTROL_HOVER_BG
        } else {
            CONTROL_CIRCLE_BG
        };
        blend_circle(buffer, buf_w, bcx, cy, r, circle_bg, CONTROL_CIRCLE_ALPHA);

        // Icon
        let fg = if hovered && is_close { CONTROL_CLOSE_HOVER_FG } else { CONTROL_FG };
        let icon_s = ICON_SIZE;
        let ix = bcx - icon_s / 2;
        let iy = cy - icon_s / 2;

        match i {
            0 => {
                // Minimize: horizontal line centered in circle
                draw_hline(buffer, buf_w, ix, cy, icon_s, fg);
            }
            1 => {
                // Maximize/Restore
                if is_maximized {
                    let s = icon_s - 2;
                    draw_rect(buffer, buf_w, ix + 2, iy, s, s, fg);
                    draw_rect(buffer, buf_w, ix, iy + 2, s, s, fg);
                    let inner_bg = circle_bg;
                    fill_rect(buffer, buf_w, ix + 1, iy + 3, s - 2, s - 2, inner_bg);
                } else {
                    draw_rect(buffer, buf_w, ix, iy, icon_s, icon_s, fg);
                }
            }
            _ => {
                // Close: X shape
                draw_x(buffer, buf_w, ix, iy, icon_s, fg);
            }
        }
    }
}

// --- Pixel drawing helpers ---

/// Fill a circle (center cx, cy; radius r) with a semi-transparent color.
#[cfg(not(target_os = "windows"))]
fn blend_circle(buffer: &mut [u32], buf_w: usize, cx: usize, cy: usize, r: usize, color: u32, alpha: u32) {
    let r2 = (r * r) as isize;
    for dy in 0..=(r * 2) {
        let py = cy + dy - r;
        let ry = dy as isize - r as isize;
        for dx in 0..=(r * 2) {
            let px = cx + dx - r;
            let rx = dx as isize - r as isize;
            if rx * rx + ry * ry <= r2 {
                if px < buf_w && py * buf_w + px < buffer.len() {
                    let idx = py * buf_w + px;
                    buffer[idx] = blend(buffer[idx], color, alpha);
                }
            }
        }
    }
}

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

/// Truncate a string to fit within `max_width` display cells, appending an
/// ellipsis if truncated. Correctly handles CJK (width 2) and won't split a
/// wide character at the boundary.
fn truncate_to_width(s: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut result = String::new();
    for c in s.chars() {
        let cw = UnicodeWidthChar::width(c).unwrap_or(0);
        if width + cw + 1 > max_width {
            result.push('\u{2026}'); // …
            return result;
        }
        width += cw;
        result.push(c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_ascii_short() {
        // Fits within limit — no truncation
        assert_eq!(truncate_to_width("hello", 10), "hello");
    }

    #[test]
    fn truncate_ascii_long() {
        // "abcdefghij" is 10 chars, limit 7 → fit 6 chars + ellipsis
        assert_eq!(truncate_to_width("abcdefghij", 7), "abcdef\u{2026}");
    }

    #[test]
    fn truncate_cjk() {
        // 5 CJK chars, display width 10, limit 7 → 3 CJK (width 6) + ellipsis (1) = 7
        assert_eq!(truncate_to_width("漢字テスト", 7), "漢字テ\u{2026}");
    }

    #[test]
    fn truncate_cjk_boundary() {
        // Limit 6: can fit 2 CJK (width 4) + ellipsis (1) = 5, but 3 CJK (6) + ellipsis (1) = 7 > 6
        assert_eq!(truncate_to_width("漢字テスト", 6), "漢字\u{2026}");
    }

    #[test]
    fn truncate_mixed() {
        // "aあb" = widths 1+2+1 = 4, limit 4 → fits without truncation
        assert_eq!(truncate_to_width("aあb", 10), "aあb");
    }

    #[test]
    fn truncate_empty() {
        assert_eq!(truncate_to_width("", 5), "");
    }
}
