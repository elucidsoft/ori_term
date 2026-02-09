use std::io::{Read, Write};
use std::thread;

use vte::ansi::CursorShape;
use winit::event_loop::EventLoopProxy;

use crate::grid::Grid;
use crate::log;
use crate::palette::Palette;
use crate::term_handler::TermHandler;
use crate::term_mode::TermMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

pub struct Tab {
    pub id: TabId,
    primary_grid: Grid,
    alt_grid: Grid,
    active_is_alt: bool,
    pub pty_writer: Option<Box<dyn Write + Send>>,
    processor: vte::ansi::Processor,
    pub title: String,
    pub palette: Palette,
    pub mode: TermMode,
    pub cursor_shape: CursorShape,
    pub pty_master: Box<dyn portable_pty::MasterPty + Send>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

#[derive(Debug)]
pub enum TermEvent {
    PtyOutput(TabId, Vec<u8>),
}

impl Tab {
    pub fn spawn(
        id: TabId,
        cols: usize,
        rows: usize,
        proxy: EventLoopProxy<TermEvent>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        log(&format!("Tab::spawn start for {:?}", id));

        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system.openpty(portable_pty::PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        log("  pty opened");

        let cmd = portable_pty::CommandBuilder::new("cmd.exe");
        let child = pair.slave.spawn_command(cmd)?;
        log("  cmd.exe spawned");
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        log("  reader/writer ready");

        let tab_id = id;
        thread::spawn(move || {
            log(&format!("reader thread started for tab {:?}", tab_id));
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        log(&format!("reader: eof for tab {:?}", tab_id));
                        break;
                    }
                    Err(e) => {
                        log(&format!("reader error for tab {:?}: {e}", tab_id));
                        break;
                    }
                    Ok(n) => {
                        let _ = proxy.send_event(TermEvent::PtyOutput(tab_id, buf[..n].to_vec()));
                    }
                }
            }
        });

        log(&format!("Tab::spawn done for {:?}", id));
        Ok(Tab {
            id,
            primary_grid: Grid::new(cols, rows),
            alt_grid: Grid::new(cols, rows),
            active_is_alt: false,
            pty_writer: Some(writer),
            processor: vte::ansi::Processor::new(),
            title: format!("Tab {}", id.0),
            palette: Palette::new(),
            mode: TermMode::default(),
            cursor_shape: CursorShape::default(),
            pty_master: pair.master,
            _child: child,
        })
    }

    pub fn grid(&self) -> &Grid {
        if self.active_is_alt { &self.alt_grid } else { &self.primary_grid }
    }

    pub fn grid_mut(&mut self) -> &mut Grid {
        if self.active_is_alt { &mut self.alt_grid } else { &mut self.primary_grid }
    }

    pub fn process_output(&mut self, data: &[u8]) {
        let mut handler = TermHandler::new(
            &mut self.primary_grid,
            &mut self.alt_grid,
            &mut self.mode,
            &mut self.palette,
            &mut self.title,
            &mut self.pty_writer,
            &mut self.active_is_alt,
            &mut self.cursor_shape,
        );
        self.processor.advance(&mut handler, data);
    }

    pub fn send_pty(&mut self, data: &[u8]) {
        if let Some(writer) = self.pty_writer.as_mut() {
            match writer.write_all(data) {
                Ok(()) => {
                    let _ = writer.flush();
                }
                Err(e) => log(&format!("send_pty ERROR for tab {:?}: {e}", self.id)),
            }
        }
    }

    pub fn resize(&mut self, cols: usize, rows: usize, pixel_width: u16, pixel_height: u16) {
        self.primary_grid.resize(cols, rows);
        self.alt_grid.resize(cols, rows);
        let _ = self.pty_master.resize(portable_pty::PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width,
            pixel_height,
        });
    }
}
