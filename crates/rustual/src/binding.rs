//! Key bindings and action strings.

use crate::events::normalize_key;
use std::collections::HashMap;

/// A key binding: one or more comma-separated keys mapped to an action string.
///
/// Action strings: `"name"` (runs on the widget that declared the binding),
/// `"app.name"`, `"screen.name"`, optionally with arguments `"name('a', 2)"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub keys: Vec<String>,
    pub action: String,
    pub description: String,
    /// Show in the footer.
    pub show: bool,
    /// Checked before the focused widget sees the key.
    pub priority: bool,
    /// Override how the key is displayed in the footer.
    pub key_display: Option<String>,
    /// Identifier for keymaps ([`App::keymap`](crate::App::keymap) / `Ctx::set_keymap`
    /// replace the keys of bindings by id). Textual: `Binding.id`.
    pub id: Option<String>,
    /// Footer group: consecutive bindings of the same group are shown as their keys followed
    /// by one description (Textual's `Binding.group`).
    pub group: Option<BindingGroup>,
    /// Set by the runtime on the footer's copy when `Widget::check_action` returns `None`
    /// (shown dimmed and not triggered).
    pub disabled: bool,
}

/// A binding group (Textual's `Binding.Group`): related bindings shown together in the
/// footer under a single description.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BindingGroup {
    pub description: String,
    /// Show the keys without spaces between them.
    pub compact: bool,
}

impl BindingGroup {
    pub fn new(description: &str) -> Self {
        BindingGroup {
            description: description.to_string(),
            compact: false,
        }
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Binding {
    pub fn new(keys: &str, action: &str, description: &str) -> Self {
        Binding {
            keys: parse_keys(keys),
            action: action.to_string(),
            description: description.to_string(),
            show: true,
            priority: false,
            key_display: None,
            id: None,
            group: None,
            disabled: false,
        }
    }

    /// Set the binding id used by keymaps (Textual: `Binding(..., id=...)`).
    pub fn id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    /// Put the binding in a footer group.
    pub fn group(mut self, group: BindingGroup) -> Self {
        self.group = Some(group);
        self
    }

    pub fn hidden(mut self) -> Self {
        self.show = false;
        self
    }

    pub fn priority(mut self) -> Self {
        self.priority = true;
        self
    }

    pub fn key_display(mut self, s: &str) -> Self {
        self.key_display = Some(s.to_string());
        self
    }

    pub fn matches(&self, key: &str) -> bool {
        self.keys.iter().any(|k| k == key)
    }

    /// Display string for the first key (`key_display`, or e.g. `"^c"` for ctrl+c, `"⏎"` for enter).
    pub fn display_key(&self) -> String {
        if let Some(d) = &self.key_display {
            return d.clone();
        }
        let key = self.keys.first().map(String::as_str).unwrap_or("");
        format_key(key)
    }
}

/// Parse a keymap key string (`"ctrl+s,f2"`) into normalized keys.
pub(crate) fn parse_keys(keys: &str) -> Vec<String> {
    keys.split(',')
        .map(normalize_key)
        .filter(|k| !k.is_empty())
        .collect()
}

/// A keymap: binding id → keys (normalized).
pub(crate) type Keymap = HashMap<String, Vec<String>>;

/// Apply a keymap to one node's bindings (Textual: `BindingsMap.apply_keymap`): bindings whose
/// id is in the keymap get its keys; other bindings lose keys that clash with a remapped one
/// (and disappear when no key is left).
pub(crate) fn apply_keymap(bindings: &mut Vec<Binding>, keymap: &Keymap) {
    if keymap.is_empty() {
        return;
    }
    let mut claimed: Vec<String> = Vec::new();
    for b in bindings.iter_mut() {
        if let Some(keys) = b.id.as_ref().and_then(|id| keymap.get(id)) {
            b.keys = keys.clone();
            claimed.extend(keys.iter().cloned());
        }
    }
    if claimed.is_empty() {
        return;
    }
    bindings.retain_mut(|b| {
        if b.id.as_ref().is_some_and(|id| keymap.contains_key(id)) {
            return true;
        }
        b.keys.retain(|k| !claimed.contains(k));
        !b.keys.is_empty()
    });
}

impl crate::app::App {
    /// Override the keys of bindings by id (Textual: `App.set_keymap`), e.g.
    /// `[("app.quit", "ctrl+x")]` for `Binding::new("q", "quit", "Quit").id("app.quit")`.
    pub fn keymap<K: Into<String>, V: AsRef<str>>(
        mut self,
        keymap: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        self.state.keymap = keymap
            .into_iter()
            .map(|(k, v)| (k.into(), parse_keys(v.as_ref())))
            .collect();
        self
    }
}

impl crate::context::Ctx<'_> {
    /// Replace the keymap (Textual: `App.set_keymap`).
    pub fn set_keymap<K: Into<String>, V: AsRef<str>>(
        &mut self,
        keymap: impl IntoIterator<Item = (K, V)>,
    ) {
        self.app.keymap = keymap
            .into_iter()
            .map(|(k, v)| (k.into(), parse_keys(v.as_ref())))
            .collect();
        self.app.dirty = true;
    }

    /// Merge entries into the keymap (Textual: `App.update_keymap`).
    pub fn update_keymap<K: Into<String>, V: AsRef<str>>(
        &mut self,
        keymap: impl IntoIterator<Item = (K, V)>,
    ) {
        for (k, v) in keymap {
            self.app.keymap.insert(k.into(), parse_keys(v.as_ref()));
        }
        self.app.dirty = true;
    }
}

/// How a key is shown in the footer, key panel and help (Textual `App.get_key_display`):
/// key names become their character or a short alias (`question_mark` → `?`, `delete` →
/// `del`, `enter` → `⏎`), `ctrl` becomes a `^` prefix on the key, and other modifiers stay in
/// front: `ctrl+q` → `^q`, `ctrl+shift+p` → `shift+^p`.
/// Key names (other than single characters) that stand for a character, from Textual's
/// `textual.keys.key_to_character` (generated with Textual 8.2.8).
const KEY_CHARACTERS: &[(&str, char)] = &[
    ("ampersand", '&'),
    ("apostrophe", '\''),
    ("asterisk", '*'),
    ("backspace", '\u{8}'),
    ("circumflex_accent", '^'),
    ("colon", ':'),
    ("comma", ','),
    ("commercial_at", '@'),
    ("delete", '\u{7f}'),
    ("dollar_sign", '$'),
    ("equals_sign", '='),
    ("escape", '\u{1b}'),
    ("exclamation_mark", '!'),
    ("full_stop", '.'),
    ("grave_accent", '`'),
    ("greater_than_sign", '>'),
    ("hyphen_minus", '-'),
    ("left_curly_bracket", '{'),
    ("left_parenthesis", '('),
    ("left_square_bracket", '['),
    ("less_than_sign", '<'),
    ("low_line", '_'),
    ("number_sign", '#'),
    ("percent_sign", '%'),
    ("plus_sign", '+'),
    ("question_mark", '?'),
    ("quotation_mark", '"'),
    ("reverse_solidus", '\\'),
    ("right_curly_bracket", '}'),
    ("right_parenthesis", ')'),
    ("right_square_bracket", ']'),
    ("semicolon", ';'),
    ("space", ' '),
    ("tab", '\t'),
    ("tilde", '~'),
    ("vertical_line", '|'),
];

/// Textual's key names (`textual.keys._character_to_key`) for characters whose name isn't in
/// [`KEY_CHARACTERS`]: the short replacements (`slash` for `solidus`, ...) and common Latin-1
/// symbols.
const MORE_KEY_CHARACTERS: &[(&str, char)] = &[
    ("slash", '/'),
    ("backslash", '\\'),
    ("at", '@'),
    ("minus", '-'),
    ("plus", '+'),
    ("underscore", '_'),
    ("solidus", '/'),
    ("pound_sign", '£'),
    ("euro_sign", '€'),
    ("section_sign", '§'),
    ("degree_sign", '°'),
    ("plus_minus_sign", '±'),
    ("multiplication_sign", '×'),
    ("division_sign", '÷'),
    ("not_sign", '¬'),
    ("broken_bar", '¦'),
    ("diaeresis", '¨'),
    ("acute_accent", '´'),
    ("cedilla", '¸'),
    ("currency_sign", '¤'),
    ("yen_sign", '¥'),
    ("copyright_sign", '©'),
    ("registered_sign", '®'),
    ("pilcrow_sign", '¶'),
    ("middle_dot", '·'),
    ("inverted_question_mark", '¿'),
    ("inverted_exclamation_mark", '¡'),
    ("left_pointing_double_angle_quotation_mark", '«'),
    ("right_pointing_double_angle_quotation_mark", '»'),
];

/// The printable character a Textual key name stands for (`slash` → `/`, `question_mark` →
/// `?`); `None` for other names, including `space`, `tab` and other non-printable keys.
pub(crate) fn name_to_character(name: &str) -> Option<char> {
    KEY_CHARACTERS
        .iter()
        .chain(MORE_KEY_CHARACTERS)
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
        .filter(|c| *c != ' ' && is_printable(*c))
}

/// The character a key produces, if any (Textual's `key_to_character`): keys with a modifier
/// have none; a single-character key is that character; named keys map to theirs (`space`,
/// `question_mark`, `slash`, …).
pub fn key_to_character(key: &str) -> Option<char> {
    if key.contains('+') && key != "+" {
        return None;
    }
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return Some(c);
    }
    KEY_CHARACTERS
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, c)| *c)
        .or_else(|| name_to_character(key))
}

/// Whether a character is printable (Python's `str.isprintable` for one character, roughly).
pub(crate) fn is_printable(c: char) -> bool {
    !c.is_control()
}

pub fn format_key(key: &str) -> String {
    if key == "+" {
        return "+".into();
    }
    let mut modifiers: Vec<&str> = key.split('+').collect();
    let name = modifiers.pop().unwrap_or_default();
    let mut display = KEY_DISPLAY
        .iter()
        .find(|(k, _)| *k == name)
        .map_or(name, |(_, d)| *d)
        .to_string();
    if let Some(i) = modifiers.iter().position(|m| *m == "ctrl") {
        modifiers.remove(i);
        display = format!("^{display}");
    }
    modifiers.push(&display);
    modifiers.join("+")
}

// Generated from Textual 8.2.8 `textual.keys.format_key` (key names whose display differs).
const KEY_DISPLAY: &[(&str, &str)] = &[
    ("ampersand", "&"),
    ("apostrophe", "'"),
    ("asterisk", "*"),
    ("at", "@"),
    ("backslash", "\\"),
    ("backspace", "⌫"),
    ("circumflex_accent", "^"),
    ("colon", ":"),
    ("comma", ","),
    ("delete", "del"),
    ("dollar_sign", "$"),
    ("down", "↓"),
    ("enter", "⏎"),
    ("equals_sign", "="),
    ("escape", "esc"),
    ("exclamation_mark", "!"),
    ("full_stop", "."),
    ("grave_accent", "`"),
    ("greater_than_sign", ">"),
    ("left", "←"),
    ("left_curly_bracket", "{"),
    ("left_parenthesis", "("),
    ("left_square_bracket", "["),
    ("less_than_sign", "<"),
    ("minus", "-"),
    ("number_sign", "#"),
    ("pagedown", "pgdn"),
    ("pageup", "pgup"),
    ("percent_sign", "%"),
    ("plus", "+"),
    ("question_mark", "?"),
    ("quotation_mark", "\""),
    ("right", "→"),
    ("right_curly_bracket", "}"),
    ("right_parenthesis", ")"),
    ("right_square_bracket", "]"),
    ("semicolon", ";"),
    ("slash", "/"),
    ("tilde", "~"),
    ("underscore", "_"),
    ("up", "↑"),
    ("vertical_line", "|"),
];

/// A parsed action string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    /// `None`, `Some("app")`, `Some("screen")` or `Some("focused")`.
    pub namespace: Option<String>,
    pub name: String,
    /// Arguments as strings with quotes stripped.
    pub args: Vec<String>,
}

impl Action {
    /// Parse `"app.quit"`, `"set_theme('nord')"`, `"scroll(1, 2)"`.
    pub fn parse(s: &str) -> Action {
        let s = s.trim();
        let (head, args) = match s.find('(') {
            Some(i) if s.ends_with(')') => (&s[..i], &s[i + 1..s.len() - 1]),
            _ => (s, ""),
        };
        let (namespace, name) = match head.split_once('.') {
            Some((ns, n)) => (Some(ns.trim().to_string()), n.trim().to_string()),
            None => (None, head.trim().to_string()),
        };
        let args = split_args(args);
        Action {
            namespace,
            name,
            args,
        }
    }
}

fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;
    for c in s.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                any = true;
            }
            None if c == ',' => {
                out.push(std::mem::take(&mut cur).trim().to_string());
                any = false;
            }
            None => {
                cur.push(c);
                if !c.is_whitespace() {
                    any = true;
                }
            }
        }
    }
    if any || !cur.trim().is_empty() || !out.is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_actions() {
        let a = Action::parse("app.set_theme('nord')");
        assert_eq!(a.namespace.as_deref(), Some("app"));
        assert_eq!(a.name, "set_theme");
        assert_eq!(a.args, vec!["nord"]);
        let b = Action::parse("scroll(1, 2)");
        assert_eq!(b.args, vec!["1", "2"]);
        assert!(Action::parse("quit").args.is_empty());
    }

    #[test]
    fn binding_keys() {
        let b = Binding::new("q,ctrl+C", "quit", "Quit");
        assert!(b.matches("ctrl+c"));
        assert!(b.matches("q"));
        assert_eq!(b.display_key(), "q");
        assert_eq!(format_key("ctrl+c"), "^c");
    }
}
