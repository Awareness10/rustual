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

/// Footer-friendly key formatting.
pub fn format_key(key: &str) -> String {
    if let Some(rest) = key.strip_prefix("ctrl+") {
        return format!("^{}", format_key(rest));
    }
    match key {
        "enter" => "⏎".into(),
        "escape" => "esc".into(),
        "space" => "space".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        "tab" => "tab".into(),
        "shift+tab" => "⇧tab".into(),
        "backspace" => "⌫".into(),
        other => other.to_string(),
    }
}

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
