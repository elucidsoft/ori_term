//! Binary entry point for the oriterm terminal emulator.

use std::io::{self, Read, Write};
use std::thread;

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

fn main() {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("failed to open pty");

    let cmd = CommandBuilder::new_default_prog();
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .expect("failed to spawn shell");

    // Drop the slave side so the reader detects EOF when the shell exits.
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .expect("failed to clone pty reader");
    let mut writer = pair
        .master
        .take_writer()
        .expect("failed to take pty writer");

    // Relay PTY output to stdout.
    let _output = thread::spawn(move || {
        let mut stdout = io::stdout();
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    if stdout.write_all(&buf[..n]).is_err() || stdout.flush().is_err() {
                        return;
                    }
                }
            }
        }
    });

    // Relay stdin to PTY input.
    let _input = thread::spawn(move || {
        let mut stdin = io::stdin();
        let mut buf = [0u8; 4096];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    if writer.write_all(&buf[..n]).is_err() {
                        return;
                    }
                }
            }
        }
    });

    // Block until the shell process exits.
    let _ = child.wait();
}
