use std::io::Write;

use unicode_width::UnicodeWidthChar;
use vte::ansi::{
    Attr, ClearMode, Color, CursorShape, CursorStyle, Handler, Hyperlink, LineClearMode,
    Mode, NamedColor, NamedMode, NamedPrivateMode, PrivateMode, Rgb, TabulationClearMode,
};

use crate::cell::CellFlags;
use crate::grid::Grid;
use crate::palette::Palette;
use crate::term_mode::TermMode;

pub struct TermHandler<'a> {
    grid: &'a mut Grid,
    alt_grid: &'a mut Grid,
    mode: &'a mut TermMode,
    palette: &'a mut Palette,
    title: &'a mut String,
    pty_writer: &'a mut Option<Box<dyn Write + Send>>,
    active_is_alt: &'a mut bool,
    cursor_shape: &'a mut CursorShape,
}

impl<'a> TermHandler<'a> {
    pub fn new(
        grid: &'a mut Grid,
        alt_grid: &'a mut Grid,
        mode: &'a mut TermMode,
        palette: &'a mut Palette,
        title: &'a mut String,
        pty_writer: &'a mut Option<Box<dyn Write + Send>>,
        active_is_alt: &'a mut bool,
        cursor_shape: &'a mut CursorShape,
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
        }
    }

    fn restore_primary_screen(&mut self, restore_cursor: bool) {
        if *self.active_is_alt {
            *self.active_is_alt = false;
            if restore_cursor {
                self.grid.restore_cursor();
            }
            self.mode.remove(TermMode::ALT_SCREEN);
        }
    }
}

impl Handler for TermHandler<'_> {
    fn input(&mut self, c: char) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        match UnicodeWidthChar::width(c) {
            Some(2) => grid.put_wide_char(c),
            Some(0) => {
                // Zero-width: attach to previous cell
                if grid.cursor.col > 0 {
                    let col = grid.cursor.col - 1;
                    let row = grid.cursor.row;
                    grid.row_mut(row)[col].push_zerowidth(c);
                }
            }
            _ => grid.put_char(c),
        }
    }

    fn goto(&mut self, line: i32, col: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        let row = if line < 0 { 0 } else { line as usize };
        grid.goto(row, col);
    }

    fn goto_line(&mut self, line: i32) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        let row = if line < 0 { 0 } else { line as usize };
        grid.goto_line(row);
    }

    fn goto_col(&mut self, col: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.goto_col(col);
    }

    fn move_up(&mut self, n: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.move_up(n);
    }

    fn move_down(&mut self, n: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.move_down(n);
    }

    fn move_forward(&mut self, n: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.move_forward(n);
    }

    fn move_backward(&mut self, n: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.move_backward(n);
    }

    fn move_down_and_cr(&mut self, n: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.move_down(n);
        grid.carriage_return();
    }

    fn move_up_and_cr(&mut self, n: usize) {
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
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.erase_display(mode);
    }

    fn clear_line(&mut self, mode: LineClearMode) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.erase_line(mode);
    }

    fn clear_tabs(&mut self, mode: TabulationClearMode) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.clear_tab_stops(mode);
    }

    fn erase_chars(&mut self, count: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.erase_chars(count);
    }

    fn delete_chars(&mut self, count: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.delete_chars(count);
    }

    fn insert_blank(&mut self, count: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.insert_blank_chars(count);
    }

    fn insert_blank_lines(&mut self, count: usize) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.insert_lines(count);
    }

    fn delete_lines(&mut self, count: usize) {
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
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.linefeed();
        if self.mode.contains(TermMode::LINE_FEED_NEW_LINE) {
            grid.carriage_return();
        }
    }

    fn carriage_return(&mut self) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.carriage_return();
    }

    fn backspace(&mut self) {
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.backspace();
    }

    fn newline(&mut self) {
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
            _ => {}
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
        let grid = if *self.active_is_alt { &mut *self.alt_grid } else { &mut *self.grid };
        grid.clear_all();
        grid.cursor.reset_attrs();
        *self.mode = TermMode::default();
        *self.active_is_alt = false;
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
}
