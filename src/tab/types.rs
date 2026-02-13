//! Shared types used across the tab subsystem.

use std::path::PathBuf;

use vte::ansi::{CharsetIndex, CursorShape, StandardCharset};
use winit::event_loop::EventLoopProxy;

/// Unique identifier for a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

/// Charset state: 4 slots (G0-G3) and an active index.
#[derive(Debug, Clone)]
pub struct CharsetState {
    pub charsets: [StandardCharset; 4],
    pub active: CharsetIndex,
}

impl Default for CharsetState {
    fn default() -> Self {
        Self {
            charsets: [StandardCharset::Ascii; 4],
            active: CharsetIndex::G0,
        }
    }
}

impl CharsetState {
    pub fn map(&self, c: char) -> char {
        let idx = match self.active {
            CharsetIndex::G0 => 0,
            CharsetIndex::G1 => 1,
            CharsetIndex::G2 => 2,
            CharsetIndex::G3 => 3,
        };
        self.charsets[idx].map(c)
    }
}

/// OSC 133 semantic prompt state.
///
/// Shell integration uses these markers to distinguish prompt, command input,
/// and command output regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptState {
    /// No prompt markers received yet or after command output completes.
    #[default]
    None,
    /// `OSC 133;A` — prompt has started (user sees the prompt).
    PromptStart,
    /// `OSC 133;B` — command input has started (user is typing).
    CommandStart,
    /// `OSC 133;C` — command output has started (command is running).
    OutputStart,
}

/// A desktop notification from OSC 9, 99, or 777.
pub struct Notification {
    pub title: String,
    pub body: String,
}

/// Configuration for spawning a new tab.
pub struct SpawnConfig {
    pub id: TabId,
    pub cols: usize,
    pub rows: usize,
    pub proxy: EventLoopProxy<TermEvent>,
    pub shell: Option<String>,
    pub max_scrollback: usize,
    pub cursor_shape: CursorShape,
    pub integration_dir: Option<PathBuf>,
    pub cwd: Option<String>,
}

/// Events sent from background threads to the event loop.
#[derive(Debug)]
pub enum TermEvent {
    PtyOutput(TabId, Vec<u8>),
    PtyExited(TabId),
    ConfigReload,
}
