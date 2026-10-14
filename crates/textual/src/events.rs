//! System events delivered to [`crate::Widget::on_event`].

use textual_render::Size;

/// A key press. `key` is a normalized Textual-style key name:
/// printable characters are themselves (`"a"`, `"A"`, `"?"`), `"space"`, `"enter"`, `"tab"`,
/// `"shift+tab"`, `"escape"`, `"backspace"`, `"delete"`, `"up"`, `"down"`, `"left"`, `"right"`,
/// `"home"`, `"end"`, `"pageup"`, `"pagedown"`, `"insert"`, `"f1"`..`"f24"`; modifiers are
/// prefixed in the order `ctrl+alt+shift+` (shift only for non-printable keys), e.g. `"ctrl+c"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: String,
    /// The printable character, if any.
    pub character: Option<char>,
}

impl KeyEvent {
    pub fn new(key: impl Into<String>) -> Self {
        let key = normalize_key(&key.into());
        let character = printable_char(&key);
        KeyEvent { key, character }
    }

    pub fn is_printable(&self) -> bool {
        self.character.is_some()
    }
}

/// Normalize a key name (lowercase modifiers, canonical modifier order, aliases such as
/// `"return"`→`"enter"`, `"esc"`→`"escape"`, `"ctrl+i"`→`"tab"`, `" "`→`"space"`).
pub fn normalize_key(key: &str) -> String {
    let key = key.trim();
    if key == " " {
        return "space".into();
    }
    if key == "+" {
        return "+".into();
    }
    let mut parts: Vec<&str> = key.split('+').collect();
    let base_raw = parts.pop().unwrap_or("");
    let base_raw = if base_raw.is_empty() && key.ends_with('+') { "+" } else { base_raw };
    let (mut ctrl, mut alt, mut shift) = (false, false, false);
    for m in parts {
        match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" | "meta" | "option" => alt = true,
            "shift" => shift = true,
            _ => {}
        }
    }
    let base = if base_raw.chars().count() == 1 {
        base_raw.to_string()
    } else {
        match base_raw.to_ascii_lowercase().as_str() {
            "return" => "enter".into(),
            "esc" => "escape".into(),
            "del" => "delete".into(),
            "pgup" | "page_up" => "pageup".into(),
            "pgdn" | "page_down" => "pagedown".into(),
            "backtab" => {
                shift = true;
                "tab".into()
            }
            other => other.to_string(),
        }
    };
    let base = if base == " " { "space".to_string() } else { base };
    // shift is implied by the character for single printable keys
    if base.chars().count() == 1 && !ctrl && !alt {
        shift = false;
    }
    let mut out = String::new();
    if ctrl {
        out.push_str("ctrl+");
    }
    if alt {
        out.push_str("alt+");
    }
    if shift {
        out.push_str("shift+");
    }
    out.push_str(&base);
    out
}

fn printable_char(key: &str) -> Option<char> {
    if key == "space" {
        return Some(' ');
    }
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_control() => Some(c),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// A mouse event. `x`/`y` are relative to the receiving widget's region (border box);
/// `screen_x`/`screen_y` are absolute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    pub x: i32,
    pub y: i32,
    pub screen_x: i32,
    pub screen_y: i32,
    pub button: Option<MouseButton>,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Key(KeyEvent),
    MouseDown(MouseEvent),
    MouseUp(MouseEvent),
    /// Down + up on the same widget.
    Click(MouseEvent),
    MouseMove(MouseEvent),
    MouseScrollUp(MouseEvent),
    MouseScrollDown(MouseEvent),
    MouseScrollLeft(MouseEvent),
    MouseScrollRight(MouseEvent),
    /// The mouse entered the widget (hover start).
    Enter,
    /// The mouse left the widget.
    Leave,
    Focus,
    Blur,
    /// The terminal was resized (sent to the app root and screens).
    Resize(Size),
    Paste(String),
}

impl Event {
    /// Whether an unhandled event bubbles to the parent widget.
    pub fn bubbles(&self) -> bool {
        matches!(
            self,
            Event::Key(_)
                | Event::MouseDown(_)
                | Event::MouseUp(_)
                | Event::Click(_)
                | Event::MouseScrollUp(_)
                | Event::MouseScrollDown(_)
                | Event::MouseScrollLeft(_)
                | Event::MouseScrollRight(_)
                | Event::Paste(_)
        )
    }

    pub fn mouse(&self) -> Option<&MouseEvent> {
        match self {
            Event::MouseDown(m)
            | Event::MouseUp(m)
            | Event::Click(m)
            | Event::MouseMove(m)
            | Event::MouseScrollUp(m)
            | Event::MouseScrollDown(m)
            | Event::MouseScrollLeft(m)
            | Event::MouseScrollRight(m) => Some(m),
            _ => None,
        }
    }

    pub fn mouse_mut(&mut self) -> Option<&mut MouseEvent> {
        match self {
            Event::MouseDown(m)
            | Event::MouseUp(m)
            | Event::Click(m)
            | Event::MouseMove(m)
            | Event::MouseScrollUp(m)
            | Event::MouseScrollDown(m)
            | Event::MouseScrollLeft(m)
            | Event::MouseScrollRight(m) => Some(m),
            _ => None,
        }
    }

    pub fn key(&self) -> Option<&KeyEvent> {
        match self {
            Event::Key(k) => Some(k),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize() {
        assert_eq!(normalize_key("Ctrl+C"), "ctrl+c");
        assert_eq!(normalize_key("shift+ctrl+up"), "ctrl+shift+up");
        assert_eq!(normalize_key("shift+A"), "A");
        assert_eq!(normalize_key("return"), "enter");
        assert_eq!(normalize_key("backtab"), "shift+tab");
        assert_eq!(normalize_key(" "), "space");
        assert_eq!(normalize_key("ctrl++"), "ctrl++");
        assert_eq!(KeyEvent::new("space").character, Some(' '));
        assert_eq!(KeyEvent::new("enter").character, None);
    }
}
