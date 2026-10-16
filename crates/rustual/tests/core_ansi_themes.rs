//! ANSI themes (`ansi-dark` / `ansi-light`), `App::ansi_color`, `:ansi` / `:nocolor` and
//! `NO_COLOR` (Textual's `ANSIToTruecolor`, `NoColor` and `Monochrome` filters).

use rustual::prelude::*;

struct Demo;
impl Widget for Demo {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Header::new(),
            Button::new("Default").id("default"),
            Button::primary("Primary").id("primary"),
            Input::new().id("input"),
            Checkbox::new("Check"),
            Switch::new(),
            Static::new("[ansi_red]red[/] text").id("text"),
            Footer::new(),
        ]
    }
}

async fn pilot(app: App) -> Pilot {
    app.test(Size::new(60, 20)).await
}

fn cell_of(p: &mut Pilot, needle: &str) -> rustual::render::Cell {
    let text = p.screen_text();
    let (y, line) = text
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(needle))
        .unwrap_or_else(|| panic!("{needle:?} not on screen:\n{text}"));
    let x = line[..line.find(needle).unwrap()].chars().count() as i32;
    p.canvas().get(x, y as i32).unwrap().clone()
}

#[tokio::test]
async fn ansi_theme_uses_native_terminal_colors() {
    let mut p = pilot(App::new(Demo).theme("ansi-dark")).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let ansi = p
        .with::<Demo, _>("App", |_, ctx| ctx.native_ansi_color())
        .await;
    assert_eq!(ansi, Some(true));
    // The screen is the terminal's own background and text color.
    let blank = p.canvas().get(59, 12).unwrap().clone();
    assert_eq!(blank.bg, Color::ANSI_DEFAULT);
    // Markup colors stay ANSI codes.
    assert_eq!(cell_of(&mut p, "red").fg, Color::from_ansi(1));
    // `:ansi` rules apply: a primary button's label is `$primary` (ansi_blue) on the default
    // background (`Button:ansi.-style-default.-primary`).
    let label = cell_of(&mut p, "Primary");
    assert_eq!(label.fg, Color::from_ansi(4));
    assert_eq!(label.bg, Color::ANSI_DEFAULT);
}

#[tokio::test]
async fn ansi_colors_convert_to_truecolor_unless_native() {
    // A truecolor theme: ANSI colors are shown with the MONOKAI terminal theme.
    let mut p = pilot(App::new(Demo)).await;
    assert_eq!(cell_of(&mut p, "red").fg, Color::rgb(244, 0, 95));
    // `:ansi` doesn't match, so buttons keep their truecolor look.
    assert_ne!(cell_of(&mut p, "Primary").bg, Color::ANSI_DEFAULT);

    // An ANSI theme with `ansi_color(false)`: converted, default colors from the terminal theme.
    let mut p = pilot(App::new(Demo).theme("ansi-dark").ansi_color(false)).await;
    let blank = p.canvas().get(59, 12).unwrap().clone();
    assert_eq!(blank.bg, Color::rgb(12, 12, 12));
    assert_eq!(cell_of(&mut p, "red").fg, Color::rgb(244, 0, 95));
    // Light themes use the ALABASTER terminal theme.
    let mut p = pilot(App::new(Demo).theme("ansi-light").ansi_color(false)).await;
    assert_eq!(
        p.canvas().get(59, 12).unwrap().bg,
        Color::rgb(247, 247, 247)
    );
    assert_eq!(cell_of(&mut p, "red").fg, Color::rgb(170, 55, 49));

    // Switching at runtime.
    let mut p = pilot(App::new(Demo).theme("ansi-dark")).await;
    p.with::<Demo, _>("App", |_, ctx| ctx.set_ansi_color(Some(false)))
        .await;
    assert_eq!(cell_of(&mut p, "red").fg, Color::rgb(244, 0, 95));
    p.with::<Demo, _>("App", |_, ctx| ctx.set_ansi_color(None))
        .await;
    assert_eq!(cell_of(&mut p, "red").fg, Color::from_ansi(1));
}

#[tokio::test]
async fn screenshots_show_ansi_colors_as_truecolor() {
    let mut p = pilot(App::new(Demo).theme("ansi-dark")).await;
    let svg = p.export_svg();
    // Native ANSI colors go through Rich's SVG export theme, as in Textual: red is #cc555a,
    // the default background #292929.
    assert!(svg.to_lowercase().contains("#cc555a"), "{svg}");
    assert!(svg.to_lowercase().contains("#292929"), "{svg}");
}

#[tokio::test]
async fn no_color() {
    // Truecolor: a gray scale.
    let mut p = pilot(App::new(Demo).no_color(true)).await;
    let c = cell_of(&mut p, "red");
    assert_eq!((c.fg.r, c.fg.r), (c.fg.g, c.fg.b));
    assert_eq!((c.bg.r, c.bg.r), (c.bg.g, c.bg.b));
    let nocolor = p.with::<Demo, _>("App", |_, ctx| ctx.no_color()).await;
    assert_eq!(nocolor, Some(true));
    // Native ANSI: the terminal's default colors only.
    let mut p = pilot(App::new(Demo).theme("ansi-dark").no_color(true)).await;
    let c = cell_of(&mut p, "red");
    assert_eq!((c.fg, c.bg), (Color::ANSI_DEFAULT, Color::ANSI_DEFAULT));
}

struct Pseudo;
impl Widget for Pseudo {
    fn compose(&mut self) -> Vec<Child> {
        compose![Static::new("x").id("s")]
    }
}

#[tokio::test]
async fn pseudo_classes() {
    let css = "#s { width: 1; height: 1; } #s:ansi { width: 2; } #s:nocolor { height: 2; }";
    let size = |p: &Pilot| {
        let id = p.query("#s")[0];
        let r = p.dom().get(id).unwrap().layout.region;
        (r.width, r.height)
    };
    let p = pilot(App::new(Pseudo).css(css).no_color(false)).await;
    assert_eq!(size(&p), (1, 1));
    let p = pilot(App::new(Pseudo).css(css).theme("ansi-light").no_color(true)).await;
    assert_eq!(size(&p), (2, 2));
    let p = pilot(App::new(Pseudo).css(css).ansi_color(true).no_color(false)).await;
    assert_eq!(size(&p), (2, 1));
}
