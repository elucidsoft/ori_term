//! Attributes, colors, hyperlinks, and cursor style.

use vte::ansi::{
    Attr, Color, CursorShape, CursorStyle, Hyperlink, NamedColor, Rgb,
};

use crate::cell::CellFlags;

use super::TermHandler;

impl TermHandler<'_> {
    #[allow(clippy::needless_pass_by_value, reason = "VTE Handler trait passes Attr by value")]
    pub(super) fn handle_terminal_attribute(&mut self, attr: Attr) {
        let grid = self.active_grid();
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

    pub(super) fn handle_set_color(&mut self, index: usize, color: Rgb) {
        self.palette.set_color(index, color);
    }

    pub(super) fn handle_reset_color(&mut self, index: usize) {
        self.palette.reset_color(index);
    }

    pub(super) fn handle_set_hyperlink(&mut self, hyperlink: Option<Hyperlink>) {
        self.active_grid().cursor.template.set_hyperlink(hyperlink);
    }

    pub(super) fn handle_set_cursor_style(&mut self, style: Option<CursorStyle>) {
        *self.cursor_shape = style.map_or_else(CursorShape::default, |s| s.shape);
    }

    pub(super) fn handle_set_cursor_shape(&mut self, shape: CursorShape) {
        *self.cursor_shape = shape;
    }

    #[allow(clippy::needless_pass_by_value, reason = "VTE Handler trait passes prefix by value")]
    pub(super) fn handle_dynamic_color_sequence(
        &self,
        prefix: String,
        index: usize,
        terminator: &str,
    ) {
        // OSC 10 = foreground, OSC 11 = background, OSC 12 = cursor color
        // When the param is "?", we respond with the current color
        let color = match index {
            0 => Some(self.palette.default_fg()),   // OSC 10
            1 => Some(self.palette.default_bg()),   // OSC 11
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
}
