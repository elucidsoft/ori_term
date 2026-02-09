/// Terminal dimensions in columns and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub width: u16,
    pub height: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            width: 80,
            height: 24,
        }
    }
}

/// Query the current terminal size, falling back to 80x24.
pub fn terminal_size() -> TerminalSize {
    crossterm::terminal::size()
        .map(|(w, h)| TerminalSize {
            width: w,
            height: h,
        })
        .unwrap_or_default()
}
