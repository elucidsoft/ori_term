use std::io::{Read, Write};
use std::thread;

use winit::event_loop::EventLoopProxy;

use crate::grid::Grid;
use crate::log;
use crate::vte_performer::Performer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

pub struct Tab {
    pub id: TabId,
    pub grid: Grid,
    pub pty_writer: Option<Box<dyn Write + Send>>,
    pub vte_parser: vte::Parser,
    pub title: String,
    // Must keep these alive — dropping the master closes the ConPTY,
    // and dropping the child may terminate the process.
    _pty_master: Box<dyn portable_pty::MasterPty + Send>,
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
            grid: Grid::new(cols, rows),
            pty_writer: Some(writer),
            vte_parser: vte::Parser::new(),
            title: format!("Tab {}", id.0),
            _pty_master: pair.master,
            _child: child,
        })
    }

    pub fn process_output(&mut self, data: &[u8]) {
        let grid = &mut self.grid;
        let writer = &mut self.pty_writer;
        let mut performer = Performer { grid, writer };
        self.vte_parser.advance(&mut performer, data);
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
}
