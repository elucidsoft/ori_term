//! VTE escape sequence handler implementation.

mod attr;
mod cursor;
mod erase;
mod input;
mod mode;
mod scroll;
mod title;

use std::io::Write;
use std::time::Instant;

use vte::ansi::{
    Attr, CharsetIndex, ClearMode, CursorShape, CursorStyle, Handler, Hyperlink,
    KeyboardModes, KeyboardModesApplyBehavior, LineClearMode, Mode, PrivateMode, Rgb,
    StandardCharset, TabulationClearMode,
};

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
    pub(super) after_zwj: bool,
    /// Row of the base cell that started this grapheme cluster.
    pub(super) base_row: usize,
    /// Column of the base cell that started this grapheme cluster.
    pub(super) base_col: usize,
}

/// VTE Handler implementation that dispatches escape sequences to grid operations.
pub struct TermHandler<'a> {
    pub(super) grid: &'a mut Grid,
    pub(super) alt_grid: &'a mut Grid,
    pub(super) mode: &'a mut TermMode,
    pub(super) palette: &'a mut Palette,
    pub(super) title: &'a mut String,
    pub(super) pty_writer: &'a mut Option<Box<dyn Write + Send>>,
    pub(super) active_is_alt: &'a mut bool,
    pub(super) cursor_shape: &'a mut CursorShape,
    pub(super) charset: &'a mut CharsetState,
    pub(super) title_stack: &'a mut Vec<String>,
    pub(super) grapheme: &'a mut GraphemeState,
    pub(super) keyboard_mode_stack: &'a mut Vec<KeyboardModes>,
    pub(super) inactive_keyboard_mode_stack: &'a mut Vec<KeyboardModes>,
    pub(super) bell_start: &'a mut Option<Instant>,
    pub(super) has_explicit_title: &'a mut bool,
    pub(super) suppress_title: &'a mut bool,
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
        bell_start: &'a mut Option<Instant>,
        has_explicit_title: &'a mut bool,
        suppress_title: &'a mut bool,
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
            bell_start,
            has_explicit_title,
            suppress_title,
        }
    }

    /// Returns a mutable reference to the currently active grid.
    pub(super) fn active_grid(&mut self) -> &mut Grid {
        if *self.active_is_alt {
            &mut *self.alt_grid
        } else {
            &mut *self.grid
        }
    }

    /// Returns a shared reference to the currently active grid.
    pub(super) fn active_grid_ref(&self) -> &Grid {
        if *self.active_is_alt {
            &*self.alt_grid
        } else {
            &*self.grid
        }
    }

    /// Apply the top of the keyboard mode stack to `TermMode`.
    pub(super) fn apply_keyboard_mode(&mut self) {
        // Clear all Kitty bits first.
        self.mode.remove(TermMode::KITTY_KEYBOARD_PROTOCOL);
        // Apply the top of the stack.
        if let Some(&top) = self.keyboard_mode_stack.last() {
            self.mode.insert(TermMode::from(top));
        }
    }

    pub(super) fn write_pty(&mut self, data: &[u8]) {
        if let Some(w) = self.pty_writer.as_mut() {
            let _ = w.write_all(data);
            let _ = w.flush();
        }
    }

    pub(super) fn swap_alt_screen(&mut self, save_cursor: bool) {
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

    pub(super) fn restore_primary_screen(&mut self, restore_cursor: bool) {
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
pub(super) fn prev_base_col(grid: &Grid) -> Option<usize> {
    use crate::cell::CellFlags;

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
    if grid.row(row)[col]
        .flags
        .contains(CellFlags::WIDE_CHAR_SPACER)
        && col > 0
    {
        Some(col - 1)
    } else {
        Some(col)
    }
}

impl Handler for TermHandler<'_> {
    fn input(&mut self, c: char) {
        self.handle_input(c);
    }

    fn goto(&mut self, line: i32, col: usize) {
        self.handle_goto(line, col);
    }

    fn goto_line(&mut self, line: i32) {
        self.handle_goto_line(line);
    }

    fn goto_col(&mut self, col: usize) {
        self.handle_goto_col(col);
    }

    fn move_up(&mut self, n: usize) {
        self.handle_move_up(n);
    }

    fn move_down(&mut self, n: usize) {
        self.handle_move_down(n);
    }

    fn move_forward(&mut self, n: usize) {
        self.handle_move_forward(n);
    }

    fn move_backward(&mut self, n: usize) {
        self.handle_move_backward(n);
    }

    fn move_down_and_cr(&mut self, n: usize) {
        self.handle_move_down_and_cr(n);
    }

    fn move_up_and_cr(&mut self, n: usize) {
        self.handle_move_up_and_cr(n);
    }

    fn save_cursor_position(&mut self) {
        self.handle_save_cursor_position();
    }

    fn restore_cursor_position(&mut self) {
        self.handle_restore_cursor_position();
    }

    fn terminal_attribute(&mut self, attr: Attr) {
        self.handle_terminal_attribute(attr);
    }

    fn set_color(&mut self, index: usize, color: Rgb) {
        self.handle_set_color(index, color);
    }

    fn reset_color(&mut self, index: usize) {
        self.handle_reset_color(index);
    }

    fn set_hyperlink(&mut self, hyperlink: Option<Hyperlink>) {
        self.handle_set_hyperlink(hyperlink);
    }

    fn set_cursor_style(&mut self, style: Option<CursorStyle>) {
        self.handle_set_cursor_style(style);
    }

    fn set_cursor_shape(&mut self, shape: CursorShape) {
        self.handle_set_cursor_shape(shape);
    }

    fn dynamic_color_sequence(&mut self, prefix: String, index: usize, terminator: &str) {
        self.handle_dynamic_color_sequence(prefix, index, terminator);
    }

    fn clear_screen(&mut self, mode: ClearMode) {
        self.handle_clear_screen(mode);
    }

    fn clear_line(&mut self, mode: LineClearMode) {
        self.handle_clear_line(mode);
    }

    fn clear_tabs(&mut self, mode: TabulationClearMode) {
        self.handle_clear_tabs(mode);
    }

    fn erase_chars(&mut self, count: usize) {
        self.handle_erase_chars(count);
    }

    fn delete_chars(&mut self, count: usize) {
        self.handle_delete_chars(count);
    }

    fn insert_blank(&mut self, count: usize) {
        self.handle_insert_blank(count);
    }

    fn insert_blank_lines(&mut self, count: usize) {
        self.handle_insert_blank_lines(count);
    }

    fn delete_lines(&mut self, count: usize) {
        self.handle_delete_lines(count);
    }

    fn scroll_up(&mut self, count: usize) {
        self.handle_scroll_up(count);
    }

    fn scroll_down(&mut self, count: usize) {
        self.handle_scroll_down(count);
    }

    fn set_scrolling_region(&mut self, top: usize, bottom: Option<usize>) {
        self.handle_set_scrolling_region(top, bottom);
    }

    fn reverse_index(&mut self) {
        self.handle_reverse_index();
    }

    fn linefeed(&mut self) {
        self.handle_linefeed();
    }

    fn carriage_return(&mut self) {
        self.handle_carriage_return();
    }

    fn backspace(&mut self) {
        self.handle_backspace();
    }

    fn newline(&mut self) {
        self.handle_newline();
    }

    fn put_tab(&mut self, count: u16) {
        self.handle_put_tab(count);
    }

    fn move_forward_tabs(&mut self, count: u16) {
        self.handle_move_forward_tabs(count);
    }

    fn move_backward_tabs(&mut self, count: u16) {
        self.handle_move_backward_tabs(count);
    }

    fn set_horizontal_tabstop(&mut self) {
        self.handle_set_horizontal_tabstop();
    }

    fn set_mode(&mut self, mode: Mode) {
        self.handle_set_mode(mode);
    }

    fn unset_mode(&mut self, mode: Mode) {
        self.handle_unset_mode(mode);
    }

    fn set_private_mode(&mut self, mode: PrivateMode) {
        self.handle_set_private_mode(mode);
    }

    fn unset_private_mode(&mut self, mode: PrivateMode) {
        self.handle_unset_private_mode(mode);
    }

    fn set_keypad_application_mode(&mut self) {
        self.handle_set_keypad_application_mode();
    }

    fn unset_keypad_application_mode(&mut self) {
        self.handle_unset_keypad_application_mode();
    }

    fn report_mode(&mut self, mode: Mode) {
        self.handle_report_mode(mode);
    }

    fn report_private_mode(&mut self, mode: PrivateMode) {
        self.handle_report_private_mode(mode);
    }

    fn device_status(&mut self, status: usize) {
        self.handle_device_status(status);
    }

    fn identify_terminal(&mut self, intermediate: Option<char>) {
        self.handle_identify_terminal(intermediate);
    }

    fn text_area_size_chars(&mut self) {
        self.handle_text_area_size_chars();
    }

    fn text_area_size_pixels(&mut self) {
        self.handle_text_area_size_pixels();
    }

    fn bell(&mut self) {
        self.handle_bell();
    }

    fn decaln(&mut self) {
        self.handle_decaln();
    }

    fn reset_state(&mut self) {
        self.handle_reset_state();
    }

    fn set_title(&mut self, title: Option<String>) {
        self.handle_set_title(title);
    }

    fn push_title(&mut self) {
        self.handle_push_title();
    }

    fn pop_title(&mut self) {
        self.handle_pop_title();
    }

    fn configure_charset(&mut self, index: CharsetIndex, charset: StandardCharset) {
        self.handle_configure_charset(index, charset);
    }

    fn set_active_charset(&mut self, index: CharsetIndex) {
        self.handle_set_active_charset(index);
    }

    fn clipboard_store(&mut self, clipboard: u8, data: &[u8]) {
        self.handle_clipboard_store(clipboard, data);
    }

    fn clipboard_load(&mut self, clipboard: u8, terminator: &str) {
        self.handle_clipboard_load(clipboard, terminator);
    }

    fn substitute(&mut self) {
        self.handle_substitute();
    }

    fn report_keyboard_mode(&mut self) {
        self.handle_report_keyboard_mode();
    }

    fn push_keyboard_mode(&mut self, mode: KeyboardModes) {
        self.handle_push_keyboard_mode(mode);
    }

    fn pop_keyboard_modes(&mut self, to_pop: u16) {
        self.handle_pop_keyboard_modes(to_pop);
    }

    fn set_keyboard_mode(&mut self, mode: KeyboardModes, behavior: KeyboardModesApplyBehavior) {
        self.handle_set_keyboard_mode(mode, behavior);
    }
}
