//! Footer: renders active bindings; pressing the mouse on an item runs its binding.

use rustual::prelude::*;
use rustual::render::Attrs;

#[derive(Default)]
struct Focusable {
    hits: usize,
}
impl Widget for Focusable {
    fn type_names(&self) -> &'static [&'static str] {
        &["Focusable"]
    }
    fn default_css(&self) -> &'static str {
        "Focusable { height: 1; }"
    }
    fn can_focus(&self) -> bool {
        true
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("x", "hit", "Hit me")]
    }
    fn run_action(&mut self, a: &str, _: &[String], _ctx: &mut Ctx) -> bool {
        if a == "hit" {
            self.hits += 1;
            return true;
        }
        false
    }
    fn render(&self, _ctx: &RenderCtx) -> Content {
        Content::plain(&format!("hits={}", self.hits))
    }
}

#[derive(Default)]
struct T {
    log: Vec<String>,
    compact: bool,
}
impl Widget for T {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Focusable::default().id("f"),
            Footer::new().compact(self.compact).id("footer")
        ]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("q", "quit", "Quit"),
            Binding::new("d", "mark", "Mark"),
            Binding::new("h", "mark", "Hidden").hidden(),
        ]
    }
    fn run_action(&mut self, a: &str, _: &[String], _ctx: &mut Ctx) -> bool {
        if a == "mark" {
            self.log.push("mark".into());
            return true;
        }
        false
    }
}

fn footer_line(pilot: &mut Pilot) -> String {
    pilot.screen_text().lines().last().unwrap().to_string()
}

#[tokio::test]
async fn footer_renders_bindings() {
    let mut pilot = App::new(T::default()).test(Size::new(60, 5)).await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    let line = footer_line(&mut pilot);
    // Focused widget first, then the app (Textual 8 `Screen.active_bindings` order).
    // The command palette key is docked on the right after a `▏` divider (as in Textual).
    assert_eq!(
        line, " x Hit me  q Quit  d Mark                       ▏^p palette",
        "{line:?}"
    );
    // Key is bold in $footer-key-foreground, the description is not.
    let c = pilot.canvas();
    let key = c.get(1, 4).unwrap();
    let desc = c.get(3, 4).unwrap();
    assert!(key.attrs.contains(Attrs::BOLD));
    assert!(!desc.attrs.contains(Attrs::BOLD));
    assert_ne!(key.fg, desc.fg);
    // Footer background ($footer-background) spans the whole row, differing from the screen.
    assert_eq!(key.bg, c.get(59, 4).unwrap().bg);
    assert_ne!(key.bg, c.get(59, 2).unwrap().bg);
}

#[tokio::test]
async fn compact_footer_has_less_whitespace() {
    let mut pilot = App::new(T {
        compact: true,
        ..Default::default()
    })
    .test(Size::new(60, 5))
    .await;
    assert_eq!(
        footer_line(&mut pilot),
        "x Hit me q Quit d Mark                           ▏^p palette"
    );
}

#[tokio::test]
async fn clicking_items_runs_bindings() {
    let mut pilot = App::new(T::default()).test(Size::new(60, 5)).await;
    // Items: " x Hit me " (0..10), " q Quit " (10..18), " d Mark " (18..26).
    // " d Mark " → non-namespaced app action.
    pilot.click_at(20, 4).await;
    assert_eq!(pilot.app::<T>().unwrap().log, vec!["mark"]);
    // A binding of the focused widget.
    pilot.click_at(2, 4).await;
    assert_eq!(pilot.query_one::<Focusable>("#f").unwrap().hits, 1);
    // Between the last item and the palette key: nothing.
    pilot.click_at(40, 4).await;
    assert!(!pilot.has_exited());
    pilot.click_at(12, 4).await;
    assert!(pilot.has_exited());
}

#[tokio::test]
async fn hovered_item_is_highlighted() {
    let mut pilot = App::new(T::default()).test(Size::new(60, 5)).await;
    let before = pilot.canvas().get(10, 4).unwrap().bg;
    pilot.click_at(10, 4).await; // moves the mouse onto "d Mark"
    let c = pilot.canvas();
    assert_ne!(c.get(10, 4).unwrap().bg, before);
    assert_eq!(c.get(2, 4).unwrap().bg, before);
    pilot.hover("#f").await;
    assert_eq!(pilot.canvas().get(10, 4).unwrap().bg, before);
}

#[tokio::test]
async fn binding_groups() {
    use rustual::binding::BindingGroup;
    #[derive(Default)]
    struct G {
        moved: Vec<String>,
    }
    impl Widget for G {
        fn compose(&mut self) -> Vec<Child> {
            compose![Footer::new()]
        }
        fn bindings(&self) -> Vec<Binding> {
            let nav = BindingGroup::new("Move");
            let zoom = BindingGroup::new("Zoom").compact(true);
            vec![
                Binding::new("q", "quit", "Quit"),
                Binding::new("h", "go('h')", "Left").group(nav.clone()),
                Binding::new("l", "go('l')", "Right").group(nav),
                Binding::new("plus", "go('+')", "In")
                    .key_display("+")
                    .group(zoom.clone()),
                Binding::new("minus", "go('-')", "Out")
                    .key_display("-")
                    .group(zoom),
                Binding::new("s", "go('s')", "Solo").group(BindingGroup::new("Alone")),
            ]
        }
        fn run_action(&mut self, a: &str, args: &[String], _: &mut Ctx) -> bool {
            if a == "go" {
                self.moved.push(args[0].clone());
                return true;
            }
            false
        }
    }
    let mut p = App::new(G::default()).test(Size::new(80, 5)).await;
    assert!(p.css_errors().is_empty());
    let line = p.screen_text().lines().last().unwrap().to_string();
    // A single-binding group is shown as a normal item.
    assert!(
        line.starts_with(" q Quit  h l Move  +- Zoom  s Solo"),
        "{line:?}"
    );
    let x = line.find(" l ").unwrap() as i32 + 1;
    p.click_at(x, 4).await;
    // The group label is not clickable.
    let label_x = line.find("Move").unwrap() as i32;
    p.click_at(label_x, 4).await;
    let minus_x = line.find('-').unwrap() as i32;
    p.click_at(minus_x, 4).await;
    assert_eq!(p.app::<G>().unwrap().moved, vec!["l", "-"]);
}

#[tokio::test]
async fn palette_key_can_be_hidden() {
    struct NoPalette;
    impl Widget for NoPalette {
        fn compose(&mut self) -> Vec<Child> {
            compose![Footer::new().show_command_palette(false)]
        }
        fn bindings(&self) -> Vec<Binding> {
            vec![Binding::new("q", "quit", "Quit")]
        }
    }
    let mut pilot = App::new(NoPalette).test(Size::new(60, 3)).await;
    assert_eq!(footer_line(&mut pilot), " q Quit");
}

struct Keys {
    hits: Vec<String>,
}
impl Widget for Keys {
    fn compose(&mut self) -> Vec<Child> {
        compose![Footer::new()]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("ctrl+q", "quit", "Quit"),
            Binding::new("delete", "hit('del')", "Delete"),
            Binding::new("question_mark", "hit('?')", "About"),
            Binding::new("ctrl+shift+p", "hit('p')", "Palette"),
            Binding::new("pagedown", "hit('pd')", "Page"),
            Binding::new("plus", "hit('+')", "Zoom"),
            Binding::new("a", "hit('a')", "Add"),
            Binding::new("shift+a", "hit('A')", "Add all"),
            Binding::new("tab", "hit('tab')", "Next"),
        ]
    }
    fn run_action(&mut self, action: &str, args: &[String], _ctx: &mut Ctx) -> bool {
        if action == "hit" {
            self.hits.push(args.join(","));
            return true;
        }
        false
    }
}

/// Key labels as Textual 8.2.8 shows them for the same bindings.
#[tokio::test]
async fn footer_shows_keys_like_textual() {
    let mut p = App::new(Keys { hits: vec![] })
        .test(Size::new(160, 3))
        .await;
    let text = p.screen_text();
    let footer = text.lines().last().unwrap();
    assert!(
        footer.starts_with(" ^q Quit  del Delete  ? About  shift+^p Palette  pgdn Page  + Zoom  a Add  shift+a Add all "),
        "{footer:?}"
    );
    // The screen's own (hidden) `tab` binding is closer to the focus and wins, as in Textual.
    assert!(!footer.contains("tab Next"), "{footer:?}");
    // `shift+a` is its own key.
    p.press(&["shift+a"]).await;
    p.press(&["a"]).await;
    assert_eq!(p.app::<Keys>().unwrap().hits, vec!["A", "a"]);
    // The app's shown `ctrl+q` replaced the built-in (hidden) one, and still quits.
    p.press(&["ctrl+q"]).await;
    assert!(p.has_exited());
}

struct Editor {
    hits: Vec<String>,
}

impl Widget for Editor {
    fn compose(&mut self) -> Vec<Child> {
        compose![Input::new().id("input"), Button::new("Go"), Footer::new()]
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("n", "hit('n')", "New"),
            Binding::new("question_mark", "hit('?')", "Help"),
            Binding::new("slash", "hit('/')", "Find"),
            Binding::new("ctrl+s", "hit('save')", "Save"),
            Binding::new("ctrl+q", "quit", "Quit"),
        ]
    }

    fn run_action(&mut self, action: &str, args: &[String], _ctx: &mut Ctx) -> bool {
        if action == "hit" {
            self.hits.extend(args.iter().cloned());
            return true;
        }
        false
    }
}

/// Keys the focused input would type are neither shown nor run (Textual's `check_consume_key`).
/// Textual still lists `slash` (it has no character for that name) although typing `/` goes to
/// the input; rustual hides it like the other printable keys.
#[tokio::test]
async fn focused_input_hides_bindings_for_printable_keys() {
    let mut p = App::new(Editor { hits: vec![] })
        .test(Size::new(100, 5))
        .await;
    assert_eq!(p.focused_id().as_deref(), Some("input"));
    let footer = footer_line(&mut p);
    assert!(footer.starts_with(" ^q Quit  ^s Save "), "{footer:?}");
    p.press(&["n", "question_mark", "slash", "ctrl+s"]).await;
    assert_eq!(p.query_one::<Input>("#input").unwrap().get_value(), "n?/");
    assert_eq!(p.app::<Editor>().unwrap().hits, vec!["save"]);
    p.press(&["tab"]).await;
    let footer = footer_line(&mut p);
    assert!(
        footer.starts_with(" ^q Quit  n New  ? Help  / Find  ^s Save "),
        "{footer:?}"
    );
}

struct Dialog;

impl Widget for Dialog {
    fn type_names(&self) -> &'static [&'static str] {
        &["Dialog"]
    }

    fn compose(&mut self) -> Vec<Child> {
        compose![Label::new("Sure?"), Footer::new()]
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("y", "app.hit('yes')", "Yes")]
    }
}

struct Base {
    hits: Vec<String>,
}

impl Widget for Base {
    fn compose(&mut self) -> Vec<Child> {
        compose![Label::new("base"), Footer::new()]
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("e", "hit('e')", "Edit"),
            Binding::new("d", "dialog", "Dialog"),
            // Without `priority` (which replaces the built-in priority binding) it wouldn't
            // reach past the modal, as in Textual.
            Binding::new("ctrl+q", "quit", "Quit").priority(),
        ]
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "hit" => self.hits.extend(args.iter().cloned()),
            "dialog" => {
                ctx.push_modal(Dialog);
            }
            _ => return false,
        }
        true
    }
}

/// As in Textual: only priority bindings reach past a modal screen, a modal's footer shows the
/// modal's bindings, and the footer beneath keeps showing its own screen's.
#[tokio::test]
async fn modal_screens_stop_the_binding_chain() {
    let mut p = App::new(Base { hits: vec![] }).test(Size::new(80, 6)).await;
    p.press(&["d"]).await;
    let text = p.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    // The dialog fills the screen here; its footer is the last line.
    // Only the modal's own binding: the app's `^p` palette binding doesn't reach past it either.
    assert_eq!(lines.last().unwrap().trim_end(), " y Yes", "{text}");
    p.press(&["e", "y"]).await;
    assert_eq!(p.app::<Base>().unwrap().hits, vec!["yes"]);
    p.press(&["ctrl+q"]).await;
    assert!(p.has_exited());
}

#[tokio::test]
async fn footer_beneath_a_modal_shows_its_own_screen() {
    let mut p = App::new(Base { hits: vec![] })
        .css("Dialog { height: 3; } Dialog Footer { display: none; }")
        .test(Size::new(80, 6))
        .await;
    p.press(&["d"]).await;
    let text = p.screen_text();
    let footer = text.lines().last().unwrap();
    assert!(footer.starts_with(" ^q Quit  e Edit  d Dialog "), "{text}");
}
