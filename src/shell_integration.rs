//! Shell integration: detect the user's shell, write integration scripts to
//! disk, and configure the PTY command environment so those scripts are
//! automatically sourced.
//!
//! Each shell gets its own injection mechanism (env vars / extra args) that
//! causes it to source our scripts on startup, emitting OSC 133 prompt
//! markers and OSC 7 CWD reports.

use std::path::{Path, PathBuf};

use portable_pty::CommandBuilder;

use crate::log;

/// Shells we know how to inject integration scripts into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    /// WSL launches the user's default login shell inside a Linux VM.
    /// Simple env vars propagate via WSLENV; shell integration is user-sourced.
    Wsl,
}

/// Detect the shell from a program path or name.
///
/// Matches on the basename, ignoring `.exe` suffix on Windows.
pub fn detect_shell(program: &str) -> Option<Shell> {
    let base = Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(program);
    let name = base.strip_suffix(".exe").unwrap_or(base);

    match name {
        "bash" => Some(Shell::Bash),
        "zsh" => Some(Shell::Zsh),
        "fish" => Some(Shell::Fish),
        "pwsh" | "powershell" => Some(Shell::PowerShell),
        "wsl" => Some(Shell::Wsl),
        _ => None,
    }
}

/// Write the embedded shell integration scripts to `base/shell-integration/`.
///
/// Returns the path to the `shell-integration/` directory on success.
/// Uses a version stamp to skip writes when scripts are already up to date.
pub fn ensure_scripts_on_disk(base: &Path) -> Result<PathBuf, std::io::Error> {
    let dir = base.join("shell-integration");
    let version = env!("CARGO_PKG_VERSION");
    let stamp_path = dir.join(".version");

    // Skip if scripts are already written for this version.
    if let Ok(existing) = std::fs::read_to_string(&stamp_path) {
        if existing.trim() == version {
            return Ok(dir);
        }
    }

    // Bash
    let bash_dir = dir.join("bash");
    std::fs::create_dir_all(&bash_dir)?;
    std::fs::write(
        bash_dir.join("oriterm.bash"),
        include_str!("../shell-integration/bash/oriterm.bash"),
    )?;
    std::fs::write(
        bash_dir.join("bash-preexec.sh"),
        include_str!("../shell-integration/bash/bash-preexec.sh"),
    )?;

    // Zsh
    let zsh_dir = dir.join("zsh");
    std::fs::create_dir_all(&zsh_dir)?;
    std::fs::write(
        zsh_dir.join(".zshenv"),
        include_str!("../shell-integration/zsh/.zshenv"),
    )?;
    std::fs::write(
        zsh_dir.join("oriterm-integration"),
        include_str!("../shell-integration/zsh/oriterm-integration"),
    )?;

    // Fish
    let fish_dir = dir.join("fish").join("vendor_conf.d");
    std::fs::create_dir_all(&fish_dir)?;
    std::fs::write(
        fish_dir.join("oriterm-shell-integration.fish"),
        include_str!("../shell-integration/fish/vendor_conf.d/oriterm-shell-integration.fish"),
    )?;

    // PowerShell
    let ps_dir = dir.join("powershell");
    std::fs::create_dir_all(&ps_dir)?;
    std::fs::write(
        ps_dir.join("oriterm.ps1"),
        include_str!("../shell-integration/powershell/oriterm.ps1"),
    )?;

    // Stamp the version so subsequent launches skip writes.
    std::fs::write(&stamp_path, version)?;

    Ok(dir)
}

/// Configure the command environment for shell integration injection.
///
/// Sets env vars on `cmd` so the target shell will source our scripts.
/// `cwd` is the inherited working directory (if any) for WSL `--cd`.
/// Returns an optional extra argument to append (e.g. `--posix` for bash).
pub fn setup_injection(
    cmd: &mut CommandBuilder,
    shell: Shell,
    integration_dir: &Path,
    cwd: Option<&str>,
) -> Option<&'static str> {
    // Common env vars for all shells.
    cmd.env("ORITERM", "1");
    cmd.env("TERM_PROGRAM", "oriterm");
    cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));

    match shell {
        Shell::Bash => {
            // Bash in --posix mode sources $ENV on startup.
            let script = integration_dir.join("bash").join("oriterm.bash");
            cmd.env("ENV", script.to_string_lossy().as_ref());
            cmd.env("ORITERM_BASH_INJECT", "1");

            // Preserve the user's HISTFILE since --posix mode may change it.
            if let Ok(histfile) = std::env::var("HISTFILE") {
                cmd.env("ORITERM_BASH_ORIG_HISTFILE", histfile);
            }

            log("shell_integration: bash injection configured");
            Some("--posix")
        }
        Shell::Zsh => {
            // Redirect ZDOTDIR so zsh sources our .zshenv first.
            let zsh_dir = integration_dir.join("zsh");

            // Save the original ZDOTDIR so our .zshenv can restore it.
            #[allow(clippy::else_if_without_else)]
            if let Ok(zdotdir) = std::env::var("ZDOTDIR") {
                cmd.env("ORITERM_ZSH_ZDOTDIR", zdotdir);
            } else if let Ok(home) = std::env::var("HOME") {
                cmd.env("ORITERM_ZSH_ZDOTDIR", home);
            }

            cmd.env("ZDOTDIR", zsh_dir.to_string_lossy().as_ref());

            log("shell_integration: zsh injection configured");
            None
        }
        Shell::Fish => {
            // Fish loads vendor_conf.d scripts from directories in XDG_DATA_DIRS.
            let fish_dir = integration_dir.join("fish");
            let existing = std::env::var("XDG_DATA_DIRS").unwrap_or_default();
            let new_val = if existing.is_empty() {
                fish_dir.to_string_lossy().into_owned()
            } else {
                format!("{}:{existing}", fish_dir.to_string_lossy())
            };
            cmd.env("XDG_DATA_DIRS", &new_val);

            log("shell_integration: fish injection configured");
            None
        }
        Shell::PowerShell => {
            // Set env var that the user's $PROFILE can check, or we can use
            // -NoExit -Command to source our script.
            let script = integration_dir.join("powershell").join("oriterm.ps1");
            cmd.env("ORITERM_PS_PROFILE", script.to_string_lossy().as_ref());

            log("shell_integration: powershell injection configured");
            None
        }
        Shell::Wsl => {
            // WSL launches the user's default login shell automatically.
            // We only propagate simple string env vars via WSLENV — no
            // path injection (ZDOTDIR etc.) across the WSL boundary.
            // Users source the integration script from their shell rc file.
            cmd.arg("--cd");
            cmd.arg(cwd.unwrap_or("~"));

            let mut wslenv = std::env::var("WSLENV").unwrap_or_default();
            if !wslenv.is_empty() {
                wslenv.push(':');
            }
            wslenv.push_str("ORITERM:TERM_PROGRAM:TERM_PROGRAM_VERSION");
            cmd.env("WSLENV", &wslenv);

            log("shell_integration: wsl configured (env vars via WSLENV, no auto-injection)");
            None
        }
    }
}

/// Convert a Windows path (e.g. `C:\Users\X\file`) to a WSL path
/// (`/mnt/c/Users/X/file`).
#[allow(dead_code)]
fn windows_to_wsl_path(path: &str) -> String {
    // Handle paths like C:\foo\bar or C:/foo/bar
    let path = path.replace('\\', "/");
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        // Safe: index 2 is right after the ASCII ':' byte.
        let rest = std::str::from_utf8(&bytes[2..]).unwrap_or_default();
        return format!("/mnt/{drive}{rest}");
    }
    // Already a Unix-style path or relative — return as-is.
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_shell_unix_paths() {
        assert_eq!(detect_shell("/usr/bin/bash"), Some(Shell::Bash));
        assert_eq!(detect_shell("/bin/zsh"), Some(Shell::Zsh));
        assert_eq!(detect_shell("/usr/local/bin/fish"), Some(Shell::Fish));
        assert_eq!(detect_shell("pwsh"), Some(Shell::PowerShell));
    }

    #[test]
    fn detect_shell_windows() {
        assert_eq!(detect_shell("bash.exe"), Some(Shell::Bash));
        assert_eq!(detect_shell("pwsh.exe"), Some(Shell::PowerShell));
        assert_eq!(detect_shell("powershell.exe"), Some(Shell::PowerShell));
    }

    #[test]
    fn detect_shell_bare_names() {
        assert_eq!(detect_shell("bash"), Some(Shell::Bash));
        assert_eq!(detect_shell("zsh"), Some(Shell::Zsh));
        assert_eq!(detect_shell("fish"), Some(Shell::Fish));
        assert_eq!(detect_shell("powershell"), Some(Shell::PowerShell));
    }

    #[test]
    fn detect_shell_wsl() {
        assert_eq!(detect_shell("wsl"), Some(Shell::Wsl));
        assert_eq!(detect_shell("wsl.exe"), Some(Shell::Wsl));
    }

    #[test]
    fn detect_shell_unknown() {
        assert_eq!(detect_shell("cmd.exe"), None);
        assert_eq!(detect_shell("sh"), None);
        assert_eq!(detect_shell("/bin/dash"), None);
        assert_eq!(detect_shell("nu"), None);
    }

    #[test]
    fn windows_to_wsl_path_drive() {
        assert_eq!(windows_to_wsl_path(r"C:\foo\bar\baz"), "/mnt/c/foo/bar/baz");
    }

    #[test]
    fn windows_to_wsl_path_unix() {
        assert_eq!(windows_to_wsl_path("/home/user/file"), "/home/user/file");
    }
}
