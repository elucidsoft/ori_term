pub mod capabilities;
pub mod size;
pub mod raw;

use std::io::{self, Write, stdout};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode},
    execute,
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::color::detect::detect_color_profile;
use crate::color::profile::ColorProfile;
use self::size::{TerminalSize, terminal_size};

/// The main entry point for terminal interaction.
pub struct Terminal {
    pub color_profile: ColorProfile,
}

impl Terminal {
    /// Create a new Terminal, auto-detecting capabilities.
    pub fn new() -> Self {
        Self {
            color_profile: detect_color_profile(),
        }
    }

    /// Query the current terminal size.
    pub fn size(&self) -> TerminalSize {
        terminal_size()
    }

    /// Enter the alternate screen, enable raw mode, and run the provided
    /// closure. Terminal state is always restored on exit, even on panic.
    pub fn run<F>(&self, f: F) -> io::Result<()>
    where
        F: FnOnce(&Self) -> io::Result<()>,
    {
        let mut out = stdout();

        // Set up panic hook to restore terminal
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = terminal::disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
            original_hook(info);
        }));

        // Enter alternate screen + raw mode
        execute!(out, EnterAlternateScreen, cursor::Hide)?;
        terminal::enable_raw_mode()?;

        // Run user code
        let result = f(self);

        // Always restore
        terminal::disable_raw_mode()?;
        execute!(out, cursor::Show, LeaveAlternateScreen)?;
        out.flush()?;

        result
    }

    /// Clear the entire screen.
    pub fn clear(&self) -> io::Result<()> {
        execute!(stdout(), terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))
    }

    /// Write a string at the given column and row.
    pub fn print_at(&self, col: u16, row: u16, text: &str) -> io::Result<()> {
        execute!(stdout(), cursor::MoveTo(col, row))?;
        print!("{text}");
        stdout().flush()
    }

    /// Block until a key is pressed. Returns the KeyCode.
    pub fn read_key(&self) -> io::Result<KeyCode> {
        loop {
            if let Event::Key(key_event) = event::read()? {
                return Ok(key_event.code);
            }
        }
    }
}
