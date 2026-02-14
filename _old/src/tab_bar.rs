//! Tab bar layout and hit-testing.

// Tab bar constants
pub const TAB_BAR_HEIGHT: usize = 46;
pub const TAB_LEFT_MARGIN: usize = 16; // space between window edge and first tab
pub(crate) const TAB_TOP_MARGIN: usize = 8; // space between window edge and tab tops
const TAB_MIN_WIDTH: usize = 80;
const TAB_MAX_WIDTH: usize = 260;
pub(crate) const TAB_PADDING: usize = 8;
pub(crate) const CLOSE_BUTTON_WIDTH: usize = 24;
pub(crate) const CLOSE_BUTTON_RIGHT_PAD: usize = 8; // padding from right edge of tab
pub const NEW_TAB_BUTTON_WIDTH: usize = 38;
pub const DROPDOWN_BUTTON_WIDTH: usize = 30;

// Window control button constants — Windows 10/11 proportions
#[cfg(target_os = "windows")]
pub(crate) const CONTROL_BUTTON_WIDTH: usize = 58;
#[cfg(target_os = "windows")]
pub const CONTROLS_ZONE_WIDTH: usize = CONTROL_BUTTON_WIDTH * 3; // 174px for 3 buttons
#[cfg(target_os = "windows")]
pub(crate) const ICON_SIZE: usize = 10;

// Window control button constants — Linux (GNOME-style circles)
#[cfg(not(target_os = "windows"))]
pub(crate) const CONTROL_BUTTON_DIAMETER: usize = 24;
#[cfg(not(target_os = "windows"))]
pub(crate) const CONTROL_BUTTON_SPACING: usize = 8;
#[cfg(not(target_os = "windows"))]
pub(crate) const CONTROL_BUTTON_MARGIN: usize = 12;
#[cfg(not(target_os = "windows"))]
pub const CONTROLS_ZONE_WIDTH: usize = CONTROL_BUTTON_MARGIN
    + 3 * CONTROL_BUTTON_DIAMETER
    + 2 * CONTROL_BUTTON_SPACING
    + CONTROL_BUTTON_MARGIN;
// = 12 + 60 + 16 + 12 = 100px
#[cfg(not(target_os = "windows"))]
pub(crate) const ICON_SIZE: usize = 8;

// Window border (Windows only — on Linux the WM draws its own border/shadow)
#[cfg(target_os = "windows")]
pub const WINDOW_BORDER_COLOR: u32 = 0x00585b70; // overlay0 accent border
#[cfg(target_os = "windows")]
pub const WINDOW_BORDER_WIDTH: usize = 1;

/// Scale a logical pixel value by DPI factor.
fn scaled(v: usize, scale: f64) -> usize {
    (v as f64 * scale).round() as usize
}

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
    pub fn compute(
        tab_count: usize,
        bar_width: usize,
        scale: f64,
        max_tab_width: Option<usize>,
    ) -> Self {
        let s = |v: usize| scaled(v, scale);
        // Reserve space for left margin, new-tab button, dropdown, and window controls
        let available = bar_width
            .saturating_sub(s(TAB_LEFT_MARGIN))
            .saturating_sub(s(NEW_TAB_BUTTON_WIDTH))
            .saturating_sub(s(DROPDOWN_BUTTON_WIDTH))
            .saturating_sub(s(CONTROLS_ZONE_WIDTH));
        let upper = max_tab_width.unwrap_or_else(|| s(TAB_MAX_WIDTH));
        let tab_width = if tab_count == 0 {
            s(TAB_MIN_WIDTH)
        } else {
            (available / tab_count).clamp(s(TAB_MIN_WIDTH), upper)
        };
        Self {
            tab_width,
            tab_count,
            bar_width,
        }
    }

    pub fn hit_test(&self, x: usize, y: usize, scale: f64) -> TabBarHit {
        let s = |v: usize| scaled(v, scale);

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
                    (
                        controls_start + s(CONTROL_BUTTON_MARGIN) + r,
                        TabBarHit::Minimize,
                    ),
                    (
                        controls_start
                            + s(CONTROL_BUTTON_MARGIN)
                            + s(CONTROL_BUTTON_DIAMETER)
                            + s(CONTROL_BUTTON_SPACING)
                            + r,
                        TabBarHit::Maximize,
                    ),
                    (
                        controls_start
                            + s(CONTROL_BUTTON_MARGIN)
                            + 2 * (s(CONTROL_BUTTON_DIAMETER) + s(CONTROL_BUTTON_SPACING))
                            + r,
                        TabBarHit::CloseWindow,
                    ),
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
                let close_start =
                    tab_right.saturating_sub(s(CLOSE_BUTTON_WIDTH) + s(CLOSE_BUTTON_RIGHT_PAD));
                if tab_x >= close_start
                    && tab_x < tab_right.saturating_sub(s(CLOSE_BUTTON_RIGHT_PAD))
                {
                    return TabBarHit::CloseTab(tab_idx);
                }
                return TabBarHit::Tab(tab_idx);
            }
        }

        // Empty space between new-tab button and controls = drag area
        TabBarHit::DragArea
    }
}
