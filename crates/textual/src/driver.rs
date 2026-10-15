//! Terminal driver built on crossterm: raw mode, alternate screen, mouse capture, bracketed
//! paste, input conversion and frame output.

use crate::app::MouseKind;
use crate::events::{KeyEvent, MouseButton, MouseEvent};
use crossterm::event::{
    self as ct, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste,
    EnableMouseCapture, EventStream, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind,
};
use crossterm::style::{
    Attribute, Color as CtColor, Print, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{self, BeginSynchronizedUpdate, EndSynchronizedUpdate};
use crossterm::{cursor, execute, queue};
use std::io::{self, Write};
use textual_render::{Attrs, Canvas, Cell, Color, Size};

/// Input converted to framework terms.
#[derive(Debug, Clone, PartialEq)]
pub enum DriverEvent {
    Key(KeyEvent),
    Mouse(MouseKind, MouseEvent),
    Paste(String),
    Resize(Size),
}

pub struct CrosstermDriver {
    out: io::BufWriter<io::Stdout>,
}

fn restore_terminal() {
    let mut out = io::stdout();
    let _ = execute!(
        out,
        DisableMouseCapture,
        DisableBracketedPaste,
        terminal::LeaveAlternateScreen,
        cursor::Show
    );
    let _ = terminal::disable_raw_mode();
}

fn enter_application_mode() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(
        out,
        terminal::EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste,
        cursor::Hide,
        terminal::Clear(terminal::ClearType::All)
    )
}

impl CrosstermDriver {
    pub fn start() -> io::Result<Self> {
        enter_application_mode()?;
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            prev(info);
        }));
        Ok(CrosstermDriver {
            out: io::BufWriter::with_capacity(1 << 16, io::stdout()),
        })
    }

    pub fn stop(&mut self) -> io::Result<()> {
        self.out.flush()?;
        restore_terminal();
        Ok(())
    }

    /// Leave application mode (alt screen, raw mode, mouse) before suspending the process.
    pub fn suspend(&mut self) -> io::Result<()> {
        self.out.flush()?;
        restore_terminal();
        Ok(())
    }

    /// Re-enter application mode after [`CrosstermDriver::suspend`] (the screen is cleared;
    /// redraw everything).
    pub fn resume(&mut self) -> io::Result<()> {
        enter_application_mode()
    }

    /// Write raw bytes (e.g. an OSC 52 escape sequence) to the terminal.
    pub fn write_raw(&mut self, s: &str) -> io::Result<()> {
        self.out.write_all(s.as_bytes())?;
        self.out.flush()
    }

    pub fn size(&self) -> Size {
        let (w, h) = terminal::size().unwrap_or((80, 24));
        Size::new(w as i32, h as i32)
    }

    pub fn event_stream(&self) -> EventStream {
        EventStream::new()
    }

    pub fn clear(&mut self) -> io::Result<()> {
        queue!(self.out, terminal::Clear(terminal::ClearType::All))
    }

    /// Write the cells of `frame` that differ from `prev` (or everything when `prev` is None).
    pub fn draw(&mut self, frame: &Canvas, prev: Option<&Canvas>) -> io::Result<()> {
        let runs = match prev {
            Some(p) => frame.diff(p),
            None => frame.full(),
        };
        if runs.is_empty() {
            return Ok(());
        }
        queue!(self.out, BeginSynchronizedUpdate)?;
        let mut last: Option<(Color, Color, Attrs)> = None;
        for run in runs {
            queue!(
                self.out,
                cursor::MoveTo(run.x.max(0) as u16, run.y.max(0) as u16)
            )?;
            for cell in &run.cells {
                if cell.symbol.is_empty() {
                    continue;
                }
                write_style(&mut self.out, cell, &mut last)?;
                queue!(self.out, Print(&cell.symbol))?;
            }
        }
        queue!(
            self.out,
            SetAttribute(Attribute::Reset),
            EndSynchronizedUpdate
        )?;
        self.out.flush()
    }
}

fn ct_color(c: Color) -> CtColor {
    CtColor::Rgb {
        r: c.r,
        g: c.g,
        b: c.b,
    }
}

fn write_style(
    out: &mut impl Write,
    cell: &Cell,
    last: &mut Option<(Color, Color, Attrs)>,
) -> io::Result<()> {
    let key = (cell.fg, cell.bg, cell.attrs);
    if *last == Some(key) {
        return Ok(());
    }
    let attrs_changed = last.map(|l| l.2 != cell.attrs).unwrap_or(true);
    if attrs_changed {
        queue!(out, SetAttribute(Attribute::Reset))?;
        let a = cell.attrs;
        let pairs = [
            (Attrs::BOLD, Attribute::Bold),
            (Attrs::DIM, Attribute::Dim),
            (Attrs::ITALIC, Attribute::Italic),
            (Attrs::UNDERLINE, Attribute::Underlined),
            (Attrs::STRIKE, Attribute::CrossedOut),
            (Attrs::REVERSE, Attribute::Reverse),
            (Attrs::BLINK, Attribute::SlowBlink),
        ];
        for (flag, attr) in pairs {
            if a.contains(flag) {
                queue!(out, SetAttribute(attr))?;
            }
        }
        queue!(
            out,
            SetForegroundColor(ct_color(cell.fg)),
            SetBackgroundColor(ct_color(cell.bg))
        )?;
    } else {
        if last.map(|l| l.0 != cell.fg).unwrap_or(true) {
            queue!(out, SetForegroundColor(ct_color(cell.fg)))?;
        }
        if last.map(|l| l.1 != cell.bg).unwrap_or(true) {
            queue!(out, SetBackgroundColor(ct_color(cell.bg)))?;
        }
    }
    *last = Some(key);
    Ok(())
}

/// Convert a crossterm key event into a Textual-style key name.
pub fn convert_key(k: ct::KeyEvent) -> Option<KeyEvent> {
    if k.kind == KeyEventKind::Release {
        return None;
    }
    let m = k.modifiers;
    let ctrl = m.contains(KeyModifiers::CONTROL);
    let alt = m.contains(KeyModifiers::ALT);
    let shift = m.contains(KeyModifiers::SHIFT);
    let base: String = match k.code {
        KeyCode::Char(c) => {
            if ctrl || alt {
                c.to_lowercase().collect()
            } else {
                return Some(KeyEvent {
                    key: if c == ' ' {
                        "space".into()
                    } else {
                        c.to_string()
                    },
                    character: Some(c),
                });
            }
        }
        KeyCode::Enter => "enter".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::BackTab => return Some(KeyEvent::new("shift+tab")),
        KeyCode::Esc => "escape".into(),
        KeyCode::Backspace => "backspace".into(),
        KeyCode::Delete => "delete".into(),
        KeyCode::Insert => "insert".into(),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        KeyCode::Home => "home".into(),
        KeyCode::End => "end".into(),
        KeyCode::PageUp => "pageup".into(),
        KeyCode::PageDown => "pagedown".into(),
        KeyCode::F(n) => format!("f{n}"),
        _ => return None,
    };
    let mut key = String::new();
    if ctrl {
        key.push_str("ctrl+");
    }
    if alt {
        key.push_str("alt+");
    }
    if shift && (base.chars().count() > 1 || ctrl || alt) {
        key.push_str("shift+");
    }
    key.push_str(&base);
    Some(KeyEvent::new(key))
}

pub fn convert_event(ev: ct::Event) -> Vec<DriverEvent> {
    match ev {
        ct::Event::Key(k) => convert_key(k).map(DriverEvent::Key).into_iter().collect(),
        ct::Event::Paste(s) => vec![DriverEvent::Paste(s)],
        ct::Event::Resize(w, h) => vec![DriverEvent::Resize(Size::new(w as i32, h as i32))],
        ct::Event::Mouse(me) => {
            let button = |b: ct::MouseButton| match b {
                ct::MouseButton::Left => MouseButton::Left,
                ct::MouseButton::Right => MouseButton::Right,
                ct::MouseButton::Middle => MouseButton::Middle,
            };
            let (kind, btn) = match me.kind {
                MouseEventKind::Down(b) => (MouseKind::Down, Some(button(b))),
                MouseEventKind::Up(b) => (MouseKind::Up, Some(button(b))),
                MouseEventKind::Drag(b) => (MouseKind::Move, Some(button(b))),
                MouseEventKind::Moved => (MouseKind::Move, None),
                MouseEventKind::ScrollUp => (MouseKind::ScrollUp, None),
                MouseEventKind::ScrollDown => (MouseKind::ScrollDown, None),
                MouseEventKind::ScrollLeft => (MouseKind::ScrollLeft, None),
                MouseEventKind::ScrollRight => (MouseKind::ScrollRight, None),
            };
            let (x, y) = (me.column as i32, me.row as i32);
            let m = MouseEvent {
                x,
                y,
                screen_x: x,
                screen_y: y,
                button: btn,
                ctrl: me.modifiers.contains(KeyModifiers::CONTROL),
                alt: me.modifiers.contains(KeyModifiers::ALT),
                shift: me.modifiers.contains(KeyModifiers::SHIFT),
            };
            vec![DriverEvent::Mouse(kind, m)]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent as CtKey;

    fn key(code: KeyCode, m: KeyModifiers) -> String {
        convert_key(CtKey::new(code, m)).unwrap().key
    }

    #[test]
    fn keys() {
        assert_eq!(key(KeyCode::Char('a'), KeyModifiers::NONE), "a");
        assert_eq!(key(KeyCode::Char('A'), KeyModifiers::SHIFT), "A");
        assert_eq!(key(KeyCode::Char('c'), KeyModifiers::CONTROL), "ctrl+c");
        assert_eq!(key(KeyCode::Char(' '), KeyModifiers::NONE), "space");
        assert_eq!(key(KeyCode::BackTab, KeyModifiers::SHIFT), "shift+tab");
        assert_eq!(key(KeyCode::Up, KeyModifiers::SHIFT), "shift+up");
        assert_eq!(key(KeyCode::F(5), KeyModifiers::NONE), "f5");
    }
}
