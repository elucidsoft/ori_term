//! Raw VTE interceptor for sequences the high-level processor drops.

use vte::Perform;

use super::types::{Notification, PromptState};

/// Raw VTE `Perform` implementation that intercepts sequences the high-level
/// `vte::ansi::Processor` drops: OSC 7 (CWD), OSC 133 (prompt markers),
/// OSC 9/99/777 (notifications), and XTVERSION (CSI > q).
pub(super) struct RawInterceptor<'a> {
    pub pty_responses: &'a mut Vec<u8>,
    pub cwd: &'a mut Option<String>,
    pub prompt_state: &'a mut PromptState,
    pub pending_notifications: &'a mut Vec<Notification>,
    pub prompt_mark_pending: &'a mut bool,
    pub has_explicit_title: &'a mut bool,
    pub suppress_title: &'a mut bool,
    pub title_dirty: &'a mut bool,
}

impl Perform for RawInterceptor<'_> {
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() || params[0].is_empty() {
            return;
        }
        match params[0] {
            // OSC 7 — Current working directory.
            // Format: OSC 7 ; file://hostname/path ST
            b"7" => {
                if params.len() >= 2 {
                    let uri = std::str::from_utf8(params[1]).unwrap_or_default();
                    // Strip file:// prefix and optional hostname to get the path.
                    let path = uri.strip_prefix("file://").map_or(uri, |rest| {
                        // Skip hostname (everything before the next /)
                        if let Some(slash) = rest.find('/') {
                            rest.split_at(slash).1
                        } else {
                            rest
                        }
                    });
                    if !path.is_empty() {
                        *self.cwd = Some(path.to_owned());
                        // CWD-based title should override ConPTY's auto-generated
                        // process title (e.g. C:\WINDOWS\system32\wsl.exe).
                        *self.has_explicit_title = false;
                        *self.suppress_title = false;
                        *self.title_dirty = true;
                    }
                }
            }
            // OSC 133 — Semantic prompt markers.
            // Format: OSC 133 ; <type>[;extras] ST
            b"133" => {
                if params.len() >= 2 && !params[1].is_empty() {
                    match params[1][0] {
                        b'A' => {
                            *self.prompt_state = PromptState::PromptStart;
                            *self.prompt_mark_pending = true;
                            *self.suppress_title = false;
                        }
                        b'B' => *self.prompt_state = PromptState::CommandStart,
                        b'C' => *self.prompt_state = PromptState::OutputStart,
                        b'D' => *self.prompt_state = PromptState::None,
                        _ => {}
                    }
                }
            }
            // OSC 9 — iTerm2 simple notification: ESC]9;body ST
            // OSC 99 — Kitty notification protocol: ESC]99;body ST
            b"9" | b"99" => {
                let body = if params.len() >= 2 {
                    String::from_utf8_lossy(params[1]).into_owned()
                } else {
                    String::new()
                };
                self.pending_notifications.push(Notification {
                    title: String::new(),
                    body,
                });
            }
            // OSC 777 — rxvt-unicode notification: ESC]777;notify;title;body ST
            b"777" => {
                if params.len() >= 2 {
                    let action = std::str::from_utf8(params[1]).unwrap_or_default();
                    if action == "notify" {
                        let title = params
                            .get(2)
                            .map(|p| String::from_utf8_lossy(p).into_owned())
                            .unwrap_or_default();
                        let body = params
                            .get(3)
                            .map(|p| String::from_utf8_lossy(p).into_owned())
                            .unwrap_or_default();
                        self.pending_notifications
                            .push(Notification { title, body });
                    }
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        _params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        // XTVERSION: CSI > q — report terminal name and version.
        if action == 'q' && intermediates == [b'>'] {
            let version = env!("CARGO_PKG_VERSION");
            let build = include_str!("../../BUILD_NUMBER").trim();
            // Response: DCS > | terminal-name(version) ST
            let response = format!("\x1bP>|oriterm({version} build {build})\x1b\\");
            self.pty_responses.extend_from_slice(response.as_bytes());
        }
    }
}
