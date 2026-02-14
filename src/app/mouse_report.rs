//! Mouse event reporting to the PTY (SGR, UTF-8, and normal encodings).

use std::io::Write as _;

use crate::tab::TabId;
use crate::term_mode::TermMode;

use super::App;

impl App {
    /// Encode and send a mouse report to the PTY.
    ///
    /// `button` is the base button code (0=left, 1=middle, 2=right, 3=release,
    /// 64=scroll-up, 65=scroll-down; add 32 for motion events).
    pub(super) fn send_mouse_report(
        &mut self,
        tab_id: TabId,
        button: u8,
        col: usize,
        line: usize,
        pressed: bool,
    ) {
        let tab = match self.tabs.get_mut(&tab_id) {
            Some(t) => t,
            None => return,
        };

        // Add modifier bits
        let mut code = button;
        if self.modifiers.shift_key() {
            code += 4;
        }
        if self.modifiers.alt_key() {
            code += 8;
        }
        if self.modifiers.control_key() {
            code += 16;
        }

        let mode = tab.mode();
        if mode.contains(TermMode::SGR_MOUSE) {
            // SGR encoding: CSI < code ; col+1 ; line+1 M/m
            let suffix = if pressed { b'M' } else { b'm' };
            let mut buf = [0u8; 32];
            let mut cursor = std::io::Cursor::new(&mut buf[..]);
            let _ = write!(cursor, "\x1b[<{code};{};{}", col + 1, line + 1);
            let pos = cursor.position() as usize;
            buf[pos] = suffix;
            tab.send_pty(&buf[..=pos]);
        } else if mode.contains(TermMode::UTF8_MOUSE) {
            // UTF-8 encoding: like normal but coordinates are UTF-8 encoded.
            // Coordinates are limited to valid Unicode scalar values (max U+10FFFF).
            let encode_utf8 = |v: u32, out: &mut [u8; 4]| -> usize {
                if let Some(c) = char::from_u32(v) {
                    c.encode_utf8(out).len()
                } else {
                    0
                }
            };
            let code_val = u32::from(code) + 32;
            let col_val = col as u32 + 1 + 32;
            let line_val = line as u32 + 1 + 32;
            // Skip report if any coordinate exceeds Unicode scalar range.
            if col_val > 0x10_FFFF || line_val > 0x10_FFFF {
                return;
            }
            let mut seq = [0u8; 15]; // ESC[M + up to 3×4 UTF-8 bytes
            seq[0] = 0x1b;
            seq[1] = b'[';
            seq[2] = b'M';
            let mut pos = 3;
            let mut tmp = [0u8; 4];
            let n = encode_utf8(code_val, &mut tmp);
            seq[pos..pos + n].copy_from_slice(&tmp[..n]);
            pos += n;
            let n = encode_utf8(col_val, &mut tmp);
            seq[pos..pos + n].copy_from_slice(&tmp[..n]);
            pos += n;
            let n = encode_utf8(line_val, &mut tmp);
            seq[pos..pos + n].copy_from_slice(&tmp[..n]);
            pos += n;
            tab.send_pty(&seq[..pos]);
        } else {
            // Normal encoding: ESC [ M Cb Cx Cy (clamp coords to 223 max)
            let cb = 32 + code;
            let cx = ((col + 1).min(223) + 32) as u8;
            let cy = ((line + 1).min(223) + 32) as u8;
            let seq = [0x1b, b'[', b'M', cb, cx, cy];
            tab.send_pty(&seq);
        }
    }
}
