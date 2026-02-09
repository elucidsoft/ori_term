/// The level of color support detected for the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColorProfile {
    /// No color support (dumb terminal, piped output, NO_COLOR set).
    None,
    /// 16 ANSI colors (4-bit).
    Ansi,
    /// 256 colors (8-bit).
    Ansi256,
    /// 24-bit true color.
    TrueColor,
}
