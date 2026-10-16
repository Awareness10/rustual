//! Terminal-level features: the OSC 52 system clipboard, suspending the process (`SIGTSTP`)
//! and saving SVG screenshots.
//!
//! Ports of `App.copy_to_clipboard`, `App.action_suspend_process` and
//! `App.action_screenshot` / `save_screenshot` (`textual/app.py`).
//!
//! Escape sequences (OSC 52) are queued here and written by the run loop after the next frame;
//! headless (Pilot) apps just keep the clipboard text (`pilot.clipboard()`).

use crate::app::{AppState, Severity};
use crate::binding::Action;
use crate::context::Ctx;
use crate::driver::CrosstermDriver;
use std::io;
use std::path::{Path, PathBuf};

/// Pending terminal writes are capped (headless apps never flush them).
const MAX_PENDING_WRITES: usize = 16;

#[derive(Default)]
pub(crate) struct TerminalState {
    /// Last text copied with `copy_to_clipboard` (Textual: `App.clipboard`).
    pub clipboard: String,
    /// Raw escape sequences to write to the terminal after the next frame.
    pub pending_writes: Vec<String>,
    /// `suspend_process` was requested; serviced by the run loop.
    pub suspend_requested: bool,
}

// ---------------------------------------------------------------------------------- clipboard

/// Standard base64 (RFC 4648, with padding).
pub(crate) fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The OSC 52 sequence that sets the system clipboard to `text`.
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

fn queue_write(state: &mut AppState, s: String) {
    let w = &mut state.terminal.pending_writes;
    w.push(s);
    if w.len() > MAX_PENDING_WRITES {
        w.remove(0);
    }
}

/// Copy `text` to the system clipboard (OSC 52) and the app's local clipboard (used by
/// `Input` / `TextArea` paste).
pub(crate) fn copy_to_clipboard(state: &mut AppState, text: &str) {
    state.terminal.clipboard = text.to_string();
    crate::widgets::input::set_clipboard_text(text);
    queue_write(state, osc52(text));
}

/// Text for `Input` / `TextArea` paste: the app clipboard (Textual: `App.clipboard`), or the
/// process-wide local clipboard if nothing was copied through the app yet.
pub(crate) fn paste_text(state: &AppState) -> String {
    if state.terminal.clipboard.is_empty() {
        crate::widgets::input::clipboard_text()
    } else {
        state.terminal.clipboard.clone()
    }
}

/// Run-loop hook: write queued escape sequences.
pub(crate) fn flush(state: &mut AppState, driver: &mut CrosstermDriver) -> io::Result<()> {
    for s in std::mem::take(&mut state.terminal.pending_writes) {
        driver.write_raw(&s)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------- suspend

/// Run-loop hook: if a suspend was requested, leave application mode, stop the process with
/// `SIGTSTP` and, once resumed (`fg`), restore the terminal and force a full redraw.
/// Returns whether the process was suspended (the caller recreates its event stream).
pub(crate) fn service_suspend(
    state: &mut AppState,
    driver: &mut CrosstermDriver,
) -> io::Result<bool> {
    if !std::mem::take(&mut state.terminal.suspend_requested) {
        return Ok(false);
    }
    if !cfg!(unix) {
        return Ok(false);
    }
    driver.suspend()?;
    send_sigtstp();
    driver.resume()?;
    state.last_frame = None;
    state.size = driver.size();
    state.dirty = true;
    Ok(true)
}

/// Stop this process (`kill -TSTP <pid>`); returns once it is continued.
fn send_sigtstp() {
    #[cfg(unix)]
    {
        let pid = std::process::id().to_string();
        let _ = std::process::Command::new("kill")
            .args(["-TSTP", &pid])
            .status();
    }
}

// ---------------------------------------------------------------------------------- screenshots

/// Days since 1970-01-01 → (year, month, day) (Howard Hinnant's `civil_from_days`).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The current time (UTC) in ISO 8601 with microseconds, like Python's `datetime.isoformat()`.
fn iso_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() as i64;
    let (y, mo, d) = civil_from_days(secs.div_euclid(86_400));
    let t = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}.{:06}",
        t / 3600,
        t % 3600 / 60,
        t % 60,
        now.subsec_micros()
    )
}

/// `generate_datetime_filename` (`textual/_files.py`): `"<prefix> <iso datetime><suffix>"` with
/// `' <>:"/\|?*.'` replaced by `_`.
pub fn datetime_filename(prefix: &str, suffix: &str) -> String {
    let stem: String = format!("{prefix} {}", iso_now())
        .chars()
        .map(|c| if " <>:\"/\\|?*.".contains(c) { '_' } else { c })
        .collect();
    stem + suffix
}

/// Save an SVG of the current frame (Textual: `save_screenshot`). `path` defaults to the
/// current directory, `filename` to [`datetime_filename`] of the app title.
pub(crate) fn save_screenshot(
    state: &mut AppState,
    filename: Option<&str>,
    path: Option<&str>,
) -> io::Result<PathBuf> {
    let title = state.env.title.clone();
    let name = match filename.filter(|f| !f.is_empty()) {
        Some(f) => f.to_string(),
        None => datetime_filename(&title, ".svg"),
    };
    let dir = path.filter(|p| !p.is_empty()).unwrap_or(".");
    let file = Path::new(dir).join(name);
    let svg = state.screenshot_frame().to_svg(&title);
    std::fs::write(&file, svg)?;
    state.dirty = true;
    Ok(file)
}

/// The `screenshot(filename?, path?)` action: save and notify (Textual: `action_screenshot`
/// + `_on_delivery_complete`).
fn screenshot_action(state: &mut AppState, action: &Action) {
    let filename = action.args.first().map(String::as_str);
    let path = action.args.get(1).map(String::as_str);
    match save_screenshot(state, filename, path) {
        Ok(file) => {
            let shown = std::fs::canonicalize(&file).unwrap_or(file);
            state.notify(
                format!("Saved screenshot to [$text-success]'{}'", shown.display()),
                "Screenshot".to_string(),
                Severity::Information,
                None,
            );
        }
        Err(e) => state.notify(
            format!("Failed to save screenshot: {e}"),
            "Screenshot not saved".to_string(),
            Severity::Error,
            None,
        ),
    }
}

// ---------------------------------------------------------------------------------- actions

/// Builtin actions provided by this module. Returns false for unknown actions.
pub(crate) fn builtin_action(state: &mut AppState, action: &Action) -> bool {
    match action.name.as_str() {
        "screenshot" => screenshot_action(state, action),
        "suspend_process" => {
            // A no-op where suspending is not supported (Textual: Windows / web).
            if cfg!(unix) {
                state.terminal.suspend_requested = true;
            }
        }
        _ => return false,
    }
    state.dirty = true;
    true
}

// ---------------------------------------------------------------------------------- Ctx API

impl Ctx<'_> {
    /// Copy `text` to the system clipboard with OSC 52 (Textual: `copy_to_clipboard`); also
    /// sets the clipboard `Input` / `TextArea` paste from.
    pub fn copy_to_clipboard(&mut self, text: &str) {
        copy_to_clipboard(self.app, text);
    }

    /// The text last copied with [`Ctx::copy_to_clipboard`] (Textual: `App.clipboard`).
    pub fn clipboard(&self) -> &str {
        &self.app.terminal.clipboard
    }

    /// Suspend the process (unix: `SIGTSTP`) after the current handler; the app restores the
    /// terminal when resumed. No-op elsewhere and in headless (test) apps.
    pub fn suspend_process(&mut self) {
        if cfg!(unix) {
            self.app.terminal.suspend_requested = true;
        }
    }

    /// Save an SVG screenshot of the current frame (Textual: `save_screenshot`); returns the path.
    pub fn save_screenshot(
        &mut self,
        filename: Option<&str>,
        path: Option<&str>,
    ) -> io::Result<PathBuf> {
        save_screenshot(self.app, filename, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(osc52("hi"), "\x1b]52;c;aGk=\x07");
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_740), (2026, 10, 14));
        let name = datetime_filename("My App", ".svg");
        assert!(name.starts_with("My_App_"), "{name}");
        assert!(name.ends_with(".svg"));
        assert_eq!(name.matches('.').count(), 1);
        assert!(!name.contains(':'));
    }
}
