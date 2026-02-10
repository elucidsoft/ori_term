use std::io::Write;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use unicode_width::UnicodeWidthChar;
use vte::ansi::{
    Attr, CharsetIndex, ClearMode, Color, CursorShape, CursorStyle, Handler, Hyperlink,
    KeyboardModes, KeyboardModesApplyBehavior, LineClearMode, Mode, NamedColor, NamedMode,
    NamedPrivateMode, PrivateMode, Rgb, StandardCharset, TabulationClearMode,
};

use crate::cell::CellFlags;
use crate::grid::Grid;
use crate::palette::Palette;
use crate::tab::CharsetState;
use crate::term_mode::TermMode;

/// Tracks grapheme cluster continuation for ZWJ emoji sequences.
///
/// When a Zero-Width Joiner (U+200D) is attached to a cell, the next
/// printable character should be attached to the same base cell as a
/// zero-width character rather than starting a new cell.
#[derive(Debug, Default)]
pub struct GraphemeState {
    /// True when the last zero-width character was ZWJ (U+200D).
    after_zwj: bool,
    /// Row of the base cell that started this grapheme cluster.
    base_row: usize,
    /// Column of the base cell that started this grapheme cluster.
    base_col: usize,
}

pub struct TermHandler<'a> {
    grid: &'a mut Grid,
    alt_grid: &'a mut Grid,
    mode: &'a mut TermMode,
    palette: &'a mut Palette,
    title: &'a mut String,
    pty_writer: &'a mut Option<Box<dyn Write + Send>>,
    active_is_alt: &'a mut bool,
    cursor_shape: &'a mut CursorShape,
    charset: &'a mut CharsetState,
    title_stack: &'a mut Vec<String>,
    grapheme: &'a mut GraphemeState,
    keyboard_mode_stack: &'a mut Vec<KeyboardModes>,
    inactive_keyboard_mode_stack: &'a mut Vec<KeyboardModes>,
}

impl<'a> TermHandler<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        grid: &'a mut Grid,
        alt_grid: &'a mut Grid,
        mode: &'a mut TermMode,
        palette: &'a mut Palette,
        title: &'a mut String,
        pty_writer: &'a mut Option<Box<dyn Write + Send>>,
        active_is_alt: &'a mut bool,
        cursor_shape: &'a mut CursorShape,
        charset: &'a mut CharsetState,
        title_stack: &'a mut Vec<String>,
        grapheme: &'a mut GraphemeState,
        keyboard_mode_stack: &'a mut Vec<KeyboardModes>,
        inactive_keyboard_mode_stack: &'a mut Vec<KeyboardModes>,
    ) -> Self {
        Self {
            grid,
            alt_grid,
            mode,
            palette,
            title,
            pty_writer,
            active_is_alt,
            cursor_shape,
            charset,
            title_stack,
            grapheme,
            keyboard_mode_stack,
            inactive_keyboard_mode_stack,
        }
    }

    /// Apply the top of the keyboard mode stack to `TermMode`.
    fn apply_keyboard_mode(&mut self) {
        // Clear all Kitty bits first.
        self.mode.remove(TermMode::KITTY_KEYBOARD_PROTOCOL);
        // Apply the top of the stack.
        if let Some(&top) = self.keyboard_mode_stack.last() {
            self.mode.insert(TermMode::from(top));
        }
    }

    fn write_pty(&mut self, data: &[u8]) {
        if let Some(w) = self.pty_writer.as_mut() {
            let _ = w.write_all(data);
            let _ = w.flush();
        }
    }

    fn swap_alt_screen(&mut self, save_cursor: bool) {
        if !*self.active_is_alt {
            // Switch to alt screen
            if save_cursor {
                self.grid.save_cursor();
            }
            *self.active_is_alt = true;
            self.alt_grid.clear_all();
            self.mode.insert(TermMode::ALT_SCREEN);
            // Save and clear keyboard mode stack for alt screen.
            std::mem::swap(self.keyboard_mode_stack, self.inactive_keyboard_mode_stack);
            self.keyboard_mode_stack.clear();
            self.apply_keyboard_mode();
        }
    }

    fn restore_primary_screen(&mut self, restore_cursor: bool) {
        if *self.active_is_alt {
            *self.active_is_alt = false;
            if restore_cursor {
                self.grid.restore_cursor();
            }
            self.mode.remove(TermMode::ALT_SCREEN);
            // Restore keyboard mode stack from primary screen.
            std::mem::swap(self.keyboard_mode_stack, self.inactive_keyboard_mode_stack);
            self.apply_keyboard_mode();
        }
    }

}

/// Find the column of the previous base cell (skipping wide char spacers).
///
/// Accounts for `input_needs_wrap`: when true, cursor.col already points
/// at the last written cell rather than the cell after it.
fn prev_base_col(grid: &Grid) -> Option<usize> {
    let row = grid.cursor.row;
    let col = if grid.cursor.input_needs_wrap {
        grid.cursor.col
    } else if grid.cursor.col > 0 {
        grid.cursor.col - 1
    } else {
        return None;
    };

    if row >= grid.lines || col >= grid.cols {
        return None;
    }

    // Skip wide char spacer to find the base cell
    if grid.row(row)[col].flags.contains(CellFlags::WIDE_CHAR_SPACER) && col > 0 {
        Some(col - 1)
    } else {
        Some(col)
    }
}

impl Handler for TermHandler<'_> {
    fn input(&mut self, c: char) {
        // Apply charset mapping (e.g., DEC Special Graphics for box-drawing)
        let c = self.charset.map(c);
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        let width = UnicodeWidthChar::width(c);


        // ZWJ continuation: after a ZWJ, the next printable char joins the cluster
        // (e.g., family emoji: 👩 + ZWJ + 👩 + ZWJ + 👧 + ZWJ + 👦)
        if self.grapheme.after_zwj {
            if let Some(w) = width {
                if w > 0 {
                    self.grapheme.after_zwj = false;
                    let row = self.grapheme.base_row;
                    let col = self.grapheme.base_col;
                    if row < grid.lines && col < grid.cols {
                        grid.row_mut(row)[col].push_zerowidth(c);
                    }
                    return;
                }
            } else {
                // None width (control char) — abandon ZWJ state
                self.grapheme.after_zwj = false;
            }
        }

        // Emoji skin tone modifiers (U+1F3FB-U+1F3FF): attach to previous wide
        // char (emoji) as zerowidth rather than occupying a new cell.
        if matches!(c, '\u{1F3FB}'..='\u{1F3FF}') {
            if let Some(prev_col) = prev_base_col(grid) {
                let row = grid.cursor.row;
                if grid.row(row)[prev_col].flags.contains(CellFlags::WIDE_CHAR) {
                    grid.row_mut(row)[prev_col].push_zerowidth(c);
                    return;
                }
            }
            // Not following a wide char — fall through to normal handling
        }

        match width {
            Some(2) => grid.put_wide_char(c),
            Some(0) => {
                // Zero-width: attach to previous cell, skipping wide char spacers.
                // When input_needs_wrap is true, cursor.col points at the cell
                // we just wrote (it was clamped back after advancing past the end).
                if let Some(col) = prev_base_col(grid) {
                    let row = grid.cursor.row;
                    grid.row_mut(row)[col].push_zerowidth(c);

                    // Track ZWJ for grapheme cluster continuation
                    if c == '\u{200D}' {
                        self.grapheme.after_zwj = true;
                        self.grapheme.base_row = row;
                        self.grapheme.base_col = col;
                    }
                }
            }
            _ => grid.put_char(c),
        }
    }

    fn goto(&mut self, line: i32, col: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        let row = if line < 0 { 0 } else { line as usize };
        grid.goto(row, col);
    }

    fn goto_line(&mut self, line: i32) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        let row = if line < 0 { 0 } else { line as usize };
        grid.goto_line(row);
    }

    fn goto_col(&mut self, col: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.goto_col(col);
    }

    fn move_up(&mut self, n: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.move_up(n);
    }

    fn move_down(&mut self, n: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.move_down(n);
    }

    fn move_forward(&mut self, n: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.move_forward(n);
    }

    fn move_backward(&mut self, n: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.move_backward(n);
    }

    fn move_down_and_cr(&mut self, n: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.move_down(n);
        grid.carriage_return();
    }

    fn move_up_and_cr(&mut self, n: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.move_up(n);
        grid.carriage_return();
    }

    fn terminal_attribute(&mut self, attr: Attr) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        let template = &mut grid.cursor.template;
        match attr {
            Attr::Reset => {
                template.fg = Color::Named(NamedColor::Foreground);
                template.bg = Color::Named(NamedColor::Background);
                template.flags = CellFlags::empty();
                template.extra = None;
            }
            Attr::Bold => template.flags.insert(CellFlags::BOLD),
            Attr::Dim => template.flags.insert(CellFlags::DIM),
            Attr::Italic => template.flags.insert(CellFlags::ITALIC),
            Attr::Underline => {
                template.flags.remove(CellFlags::ANY_UNDERLINE);
                template.flags.insert(CellFlags::UNDERLINE);
            }
            Attr::DoubleUnderline => {
                template.flags.remove(CellFlags::ANY_UNDERLINE);
                template.flags.insert(CellFlags::DOUBLE_UNDERLINE);
            }
            Attr::Undercurl => {
                template.flags.remove(CellFlags::ANY_UNDERLINE);
                template.flags.insert(CellFlags::UNDERCURL);
            }
            Attr::DottedUnderline => {
                template.flags.remove(CellFlags::ANY_UNDERLINE);
                template.flags.insert(CellFlags::DOTTED_UNDERLINE);
            }
            Attr::DashedUnderline => {
                template.flags.remove(CellFlags::ANY_UNDERLINE);
                template.flags.insert(CellFlags::DASHED_UNDERLINE);
            }
            Attr::BlinkSlow | Attr::BlinkFast => template.flags.insert(CellFlags::BLINK),
            Attr::Reverse => template.flags.insert(CellFlags::INVERSE),
            Attr::Hidden => template.flags.insert(CellFlags::HIDDEN),
            Attr::Strike => template.flags.insert(CellFlags::STRIKEOUT),
            Attr::CancelBold => template.flags.remove(CellFlags::BOLD),
            Attr::CancelBoldDim => {
                template.flags.remove(CellFlags::BOLD);
                template.flags.remove(CellFlags::DIM);
            }
            Attr::CancelItalic => template.flags.remove(CellFlags::ITALIC),
            Attr::CancelUnderline => template.flags.remove(CellFlags::ANY_UNDERLINE),
            Attr::CancelBlink => template.flags.remove(CellFlags::BLINK),
            Attr::CancelReverse => template.flags.remove(CellFlags::INVERSE),
            Attr::CancelHidden => template.flags.remove(CellFlags::HIDDEN),
            Attr::CancelStrike => template.flags.remove(CellFlags::STRIKEOUT),
            Attr::Foreground(color) => template.fg = color,
            Attr::Background(color) => template.bg = color,
            Attr::UnderlineColor(color) => {
                template.set_underline_color(color);
            }
        }
    }

    fn clear_screen(&mut self, mode: ClearMode) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.erase_display(mode);
    }

    fn clear_line(&mut self, mode: LineClearMode) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.erase_line(mode);
    }

    fn clear_tabs(&mut self, mode: TabulationClearMode) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.clear_tab_stops(mode);
    }

    fn erase_chars(&mut self, count: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.erase_chars(count);
    }

    fn delete_chars(&mut self, count: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.delete_chars(count);
    }

    fn insert_blank(&mut self, count: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.insert_blank_chars(count);
    }

    fn insert_blank_lines(&mut self, count: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.insert_lines(count);
    }

    fn delete_lines(&mut self, count: usize) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.delete_lines(count);
    }

    fn scroll_up(&mut self, count: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.scroll_up(count);
    }

    fn scroll_down(&mut self, count: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.scroll_down(count);
    }

    fn set_scrolling_region(&mut self, top: usize, bottom: Option<usize>) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.set_scroll_region(top, bottom);
        // Cursor moves to home after DECSTBM
        grid.goto(0, 0);
    }

    fn reverse_index(&mut self) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.reverse_index();
    }

    fn linefeed(&mut self) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.linefeed();
        if self.mode.contains(TermMode::LINE_FEED_NEW_LINE) {
            grid.carriage_return();
        }
    }

    fn carriage_return(&mut self) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.carriage_return();
    }

    fn backspace(&mut self) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.backspace();
    }

    fn newline(&mut self) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.linefeed();
        grid.carriage_return();
    }

    fn put_tab(&mut self, count: u16) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.advance_tab(count);
    }

    fn move_forward_tabs(&mut self, count: u16) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.advance_tab(count);
    }

    fn move_backward_tabs(&mut self, count: u16) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.backward_tab(count);
    }

    fn set_horizontal_tabstop(&mut self) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.set_tab_stop();
    }

    fn save_cursor_position(&mut self) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.save_cursor();
    }

    fn restore_cursor_position(&mut self) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.restore_cursor();
    }

    fn set_title(&mut self, title: Option<String>) {
        if let Some(t) = title {
            *self.title = t;
        }
    }

    fn device_status(&mut self, status: usize) {
        match status {
            // DSR 5 — Device Status Report: respond "OK"
            5 => {
                self.write_pty(b"\x1b[0n");
            }
            // DSR 6 — Cursor Position Report
            6 => {
                let grid = if *self.active_is_alt { &*self.alt_grid } else { &*self.grid };
                let response = format!(
                    "\x1b[{};{}R",
                    grid.cursor.row + 1,
                    grid.cursor.col + 1,
                );
                self.write_pty(response.as_bytes());
            }
            _ => {}
        }
    }

    fn identify_terminal(&mut self, intermediate: Option<char>) {
        match intermediate {
            // DA2 — Secondary Device Attributes (CSI > c)
            Some('>') => {
                // Report as VT220-compatible: type 1, firmware version 100, ROM 0
                self.write_pty(b"\x1b[>1;100;0c");
            }
            // DA — Primary Device Attributes (CSI c or ESC Z)
            _ => {
                // Report VT220 with ANSI color (62), columns (1), sixel (4), selective erase (6)
                self.write_pty(b"\x1b[?62;22c");
            }
        }
    }

    fn report_mode(&mut self, mode: Mode) {
        // DECRPM response: CSI Ps; Pm $ y
        // Pm: 1 = set, 2 = reset, 0 = not recognized
        let (param, state) = match mode {
            Mode::Named(NamedMode::Insert) => {
                (4, if self.mode.contains(TermMode::INSERT) { 1 } else { 2 })
            }
            Mode::Named(NamedMode::LineFeedNewLine) => {
                (20, if self.mode.contains(TermMode::LINE_FEED_NEW_LINE) { 1 } else { 2 })
            }
            Mode::Unknown(n) => (n as u32, 0u8),
        };
        let response = format!("\x1b[{param};{state}$y");
        self.write_pty(response.as_bytes());
    }

    fn report_private_mode(&mut self, mode: PrivateMode) {
        // DECRPM response: CSI ? Ps; Pm $ y
        let (param, state) = match mode {
            PrivateMode::Named(named) => {
                let flag = match named {
                    NamedPrivateMode::CursorKeys => (1, TermMode::APP_CURSOR),
                    NamedPrivateMode::Origin => (6, TermMode::ORIGIN),
                    NamedPrivateMode::LineWrap => (7, TermMode::LINE_WRAP),
                    NamedPrivateMode::ShowCursor => (25, TermMode::SHOW_CURSOR),
                    NamedPrivateMode::ReportMouseClicks => (1000, TermMode::MOUSE_REPORT),
                    NamedPrivateMode::ReportCellMouseMotion => (1002, TermMode::MOUSE_MOTION),
                    NamedPrivateMode::ReportAllMouseMotion => (1003, TermMode::MOUSE_ALL),
                    NamedPrivateMode::ReportFocusInOut => (1004, TermMode::FOCUS_IN_OUT),
                    NamedPrivateMode::Utf8Mouse => (1005, TermMode::UTF8_MOUSE),
                    NamedPrivateMode::SgrMouse => (1006, TermMode::SGR_MOUSE),
                    NamedPrivateMode::AlternateScroll => (1007, TermMode::ALTERNATE_SCROLL),
                    NamedPrivateMode::BracketedPaste => (2004, TermMode::BRACKETED_PASTE),
                    NamedPrivateMode::SwapScreenAndSetRestoreCursor => (1049, TermMode::ALT_SCREEN),
                    _ => return,
                };
                (flag.0, if self.mode.contains(flag.1) { 1u8 } else { 2 })
            }
            PrivateMode::Unknown(n) => (n as u32, 0u8),
        };
        let response = format!("\x1b[?{param};{state}$y");
        self.write_pty(response.as_bytes());
    }

    fn set_mode(&mut self, mode: Mode) {
        match mode {
            Mode::Named(NamedMode::Insert) => self.mode.insert(TermMode::INSERT),
            Mode::Named(NamedMode::LineFeedNewLine) => self.mode.insert(TermMode::LINE_FEED_NEW_LINE),
            _ => {}
        }
    }

    fn unset_mode(&mut self, mode: Mode) {
        match mode {
            Mode::Named(NamedMode::Insert) => self.mode.remove(TermMode::INSERT),
            Mode::Named(NamedMode::LineFeedNewLine) => self.mode.remove(TermMode::LINE_FEED_NEW_LINE),
            _ => {}
        }
    }

    fn set_private_mode(&mut self, mode: PrivateMode) {
        match mode {
            PrivateMode::Named(NamedPrivateMode::CursorKeys) => {
                self.mode.insert(TermMode::APP_CURSOR);
            }
            PrivateMode::Named(NamedPrivateMode::Origin) => {
                self.mode.insert(TermMode::ORIGIN);
            }
            PrivateMode::Named(NamedPrivateMode::LineWrap) => {
                self.mode.insert(TermMode::LINE_WRAP);
            }
            PrivateMode::Named(NamedPrivateMode::ShowCursor) => {
                self.mode.insert(TermMode::SHOW_CURSOR);
            }
            PrivateMode::Named(NamedPrivateMode::ReportMouseClicks) => {
                self.mode.insert(TermMode::MOUSE_REPORT);
            }
            PrivateMode::Named(NamedPrivateMode::ReportCellMouseMotion) => {
                self.mode.insert(TermMode::MOUSE_MOTION);
            }
            PrivateMode::Named(NamedPrivateMode::ReportAllMouseMotion) => {
                self.mode.insert(TermMode::MOUSE_ALL);
            }
            PrivateMode::Named(NamedPrivateMode::ReportFocusInOut) => {
                self.mode.insert(TermMode::FOCUS_IN_OUT);
            }
            PrivateMode::Named(NamedPrivateMode::SgrMouse) => {
                self.mode.insert(TermMode::SGR_MOUSE);
            }
            PrivateMode::Named(NamedPrivateMode::Utf8Mouse) => {
                self.mode.insert(TermMode::UTF8_MOUSE);
            }
            PrivateMode::Named(NamedPrivateMode::AlternateScroll) => {
                self.mode.insert(TermMode::ALTERNATE_SCROLL);
            }
            PrivateMode::Named(NamedPrivateMode::BracketedPaste) => {
                self.mode.insert(TermMode::BRACKETED_PASTE);
            }
            PrivateMode::Named(NamedPrivateMode::SwapScreenAndSetRestoreCursor) => {
                self.swap_alt_screen(true);
            }
            // SyncUpdate (mode 2026): handled by vte Processor internally
            _ => {}
        }
    }

    fn unset_private_mode(&mut self, mode: PrivateMode) {
        match mode {
            PrivateMode::Named(NamedPrivateMode::CursorKeys) => {
                self.mode.remove(TermMode::APP_CURSOR);
            }
            PrivateMode::Named(NamedPrivateMode::Origin) => {
                self.mode.remove(TermMode::ORIGIN);
            }
            PrivateMode::Named(NamedPrivateMode::LineWrap) => {
                self.mode.remove(TermMode::LINE_WRAP);
            }
            PrivateMode::Named(NamedPrivateMode::ShowCursor) => {
                self.mode.remove(TermMode::SHOW_CURSOR);
            }
            PrivateMode::Named(NamedPrivateMode::ReportMouseClicks) => {
                self.mode.remove(TermMode::MOUSE_REPORT);
            }
            PrivateMode::Named(NamedPrivateMode::ReportCellMouseMotion) => {
                self.mode.remove(TermMode::MOUSE_MOTION);
            }
            PrivateMode::Named(NamedPrivateMode::ReportAllMouseMotion) => {
                self.mode.remove(TermMode::MOUSE_ALL);
            }
            PrivateMode::Named(NamedPrivateMode::ReportFocusInOut) => {
                self.mode.remove(TermMode::FOCUS_IN_OUT);
            }
            PrivateMode::Named(NamedPrivateMode::SgrMouse) => {
                self.mode.remove(TermMode::SGR_MOUSE);
            }
            PrivateMode::Named(NamedPrivateMode::Utf8Mouse) => {
                self.mode.remove(TermMode::UTF8_MOUSE);
            }
            PrivateMode::Named(NamedPrivateMode::AlternateScroll) => {
                self.mode.remove(TermMode::ALTERNATE_SCROLL);
            }
            PrivateMode::Named(NamedPrivateMode::BracketedPaste) => {
                self.mode.remove(TermMode::BRACKETED_PASTE);
            }
            PrivateMode::Named(NamedPrivateMode::SwapScreenAndSetRestoreCursor) => {
                self.restore_primary_screen(true);
            }
            // SyncUpdate (mode 2026): handled by vte Processor internally
            _ => {}
        }
    }

    fn dynamic_color_sequence(&mut self, prefix: String, index: usize, terminator: &str) {
        // OSC 10 = foreground, OSC 11 = background, OSC 12 = cursor color
        // When the param is "?", we respond with the current color
        let color = match index {
            0 => Some(self.palette.default_fg()),  // OSC 10
            1 => Some(self.palette.default_bg()),  // OSC 11
            2 => Some(self.palette.cursor_color()), // OSC 12
            _ => None,
        };
        if let Some(rgb) = color {
            // Respond in XParseColor format: rgb:RRRR/GGGG/BBBB (16-bit per channel)
            let response = format!(
                "\x1b]{prefix};rgb:{:04x}/{:04x}/{:04x}{terminator}",
                (rgb.r as u16) << 8 | rgb.r as u16,
                (rgb.g as u16) << 8 | rgb.g as u16,
                (rgb.b as u16) << 8 | rgb.b as u16,
            );
            self.write_pty(response.as_bytes());
        }
    }

    fn set_color(&mut self, index: usize, color: Rgb) {
        self.palette.set_color(index, color);
    }

    fn reset_color(&mut self, index: usize) {
        self.palette.reset_color(index);
    }

    fn set_hyperlink(&mut self, hyperlink: Option<Hyperlink>) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.cursor.template.set_hyperlink(hyperlink);
    }

    fn set_cursor_style(&mut self, style: Option<CursorStyle>) {
        if let Some(s) = style {
            *self.cursor_shape = s.shape;
        } else {
            *self.cursor_shape = CursorShape::default();
        }
    }

    fn set_cursor_shape(&mut self, shape: CursorShape) {
        *self.cursor_shape = shape;
    }

    fn decaln(&mut self) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.decaln();
    }

    fn reset_state(&mut self) {
        self.grapheme.after_zwj = false;
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.clear_all();
        grid.cursor.reset_attrs();
        *self.mode = TermMode::default();
        *self.active_is_alt = false;
        self.keyboard_mode_stack.clear();
    }

    fn set_keypad_application_mode(&mut self) {
        self.mode.insert(TermMode::APP_KEYPAD);
    }

    fn unset_keypad_application_mode(&mut self) {
        self.mode.remove(TermMode::APP_KEYPAD);
    }

    fn bell(&mut self) {
        // Could emit a system bell sound — skip for now
    }

    fn text_area_size_chars(&mut self) {
        let grid = if *self.active_is_alt { &*self.alt_grid } else { &*self.grid };
        let response = format!("\x1b[8;{};{}t", grid.lines, grid.cols);
        self.write_pty(response.as_bytes());
    }

    fn text_area_size_pixels(&mut self) {
        // Report pixel size as CSI 4 ; height ; width t
        // We don't track pixel size in the handler, so report character-based estimate
        let grid = if *self.active_is_alt { &*self.alt_grid } else { &*self.grid };
        // Approximate: 8px per col, 16px per row (common monospace metrics)
        let response = format!("\x1b[4;{};{}t", grid.lines * 16, grid.cols * 8);
        self.write_pty(response.as_bytes());
    }

    fn configure_charset(&mut self, index: CharsetIndex, charset: StandardCharset) {
        let slot = match index {
            CharsetIndex::G0 => 0,
            CharsetIndex::G1 => 1,
            CharsetIndex::G2 => 2,
            CharsetIndex::G3 => 3,
        };
        self.charset.charsets[slot] = charset;
    }

    fn set_active_charset(&mut self, index: CharsetIndex) {
        self.charset.active = index;
    }

    fn push_title(&mut self) {
        self.title_stack.push(self.title.clone());
    }

    fn pop_title(&mut self) {
        if let Some(t) = self.title_stack.pop() {
            *self.title = t;
        }
    }

    fn clipboard_store(&mut self, _clipboard: u8, data: &[u8]) {
        // OSC 52 clipboard store: data is base64-encoded text from the application.
        // Selector byte (_clipboard) maps c/p/s — all go to system clipboard on Windows.
        if let Ok(decoded) = BASE64.decode(data) {
            if let Ok(text) = String::from_utf8(decoded) {
                crate::clipboard::set_text(&text);
            }
        }
    }

    fn clipboard_load(&mut self, _clipboard: u8, terminator: &str) {
        // OSC 52 clipboard load: respond with base64-encoded clipboard contents.
        // Format: ESC ] 52 ; <selector> ; <base64> <terminator>
        if let Some(text) = crate::clipboard::get_text() {
            let encoded = BASE64.encode(text.as_bytes());
            let response = format!("\x1b]52;c;{encoded}{terminator}");
            self.write_pty(response.as_bytes());
        }
    }

    fn substitute(&mut self) {
        // SUB — treated as a space character
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.put_char(' ');
    }

    fn report_keyboard_mode(&mut self) {
        let bits = self.keyboard_mode_stack.last().copied().unwrap_or(KeyboardModes::NO_MODE);
        let response = format!("\x1b[?{}u", bits.bits());
        self.write_pty(response.as_bytes());
    }

    fn push_keyboard_mode(&mut self, mode: KeyboardModes) {
        self.keyboard_mode_stack.push(mode);
        self.apply_keyboard_mode();
    }

    fn pop_keyboard_modes(&mut self, to_pop: u16) {
        let to_pop = (to_pop as usize).min(self.keyboard_mode_stack.len());
        let new_len = self.keyboard_mode_stack.len() - to_pop;
        self.keyboard_mode_stack.truncate(new_len);
        self.apply_keyboard_mode();
    }

    fn set_keyboard_mode(&mut self, mode: KeyboardModes, behavior: KeyboardModesApplyBehavior) {
        let current = self.keyboard_mode_stack.last().copied().unwrap_or(KeyboardModes::NO_MODE);
        let new_mode = match behavior {
            KeyboardModesApplyBehavior::Replace => mode,
            KeyboardModesApplyBehavior::Union => current | mode,
            KeyboardModesApplyBehavior::Difference => current & !mode,
        };
        if let Some(top) = self.keyboard_mode_stack.last_mut() {
            *top = new_mode;
        } else {
            self.keyboard_mode_stack.push(new_mode);
        }
        self.apply_keyboard_mode();
    }
}
