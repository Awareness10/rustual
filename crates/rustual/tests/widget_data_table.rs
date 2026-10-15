//! DataTable tests driven by the Pilot.

use rustual::prelude::*;
use rustual::widgets::data_table::{
    CellHighlighted, CellSelected, ColumnSelected, HeaderSelected, RowHighlighted, RowSelected,
};
use std::time::Instant;

#[derive(Default)]
struct TableApp {
    rows: usize,
    cursor: Option<CursorType>,
    events: Vec<String>,
}

fn make_table(rows: usize, cursor: CursorType) -> DataTable {
    let mut t = DataTable::new().cursor_type(cursor).zebra_stripes(true);
    t.add_column_with("Name", None, Some("name")).unwrap();
    t.add_column_with("Num", None, Some("num")).unwrap();
    t.add_column_with("Wide", Some(30), Some("wide")).unwrap();
    for i in 0..rows {
        t.add_row_with(
            [
                CellValue::from(format!("row{i}")),
                CellValue::from((i * 7) % 13),
                CellValue::from(format!("w{i}")),
            ],
            Some(1),
            Some(&format!("k{i}")),
            None,
        )
        .unwrap();
    }
    t
}

impl Widget for TableApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![make_table(self.rows, self.cursor.unwrap_or(CursorType::Cell)).id("t")]
    }
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(h) = m.get::<CellHighlighted>() {
            self.events.push(format!(
                "cell-hl {} {}",
                h.coordinate.row, h.coordinate.column
            ));
        }
        if let Some(h) = m.get::<CellSelected>() {
            self.events.push(format!(
                "cell-sel {} {}",
                h.value,
                h.cell_key.row_key.value()
            ));
        }
        if let Some(h) = m.get::<RowHighlighted>() {
            self.events.push(format!("row-hl {}", h.cursor_row));
        }
        if let Some(h) = m.get::<RowSelected>() {
            self.events
                .push(format!("row-sel {} {}", h.cursor_row, h.row_key.value()));
        }
        if let Some(h) = m.get::<ColumnSelected>() {
            self.events
                .push(format!("col-sel {}", h.column_key.value()));
        }
        if let Some(h) = m.get::<HeaderSelected>() {
            self.events.push(format!("header {}", h.column_key.value()));
        }
        Flow::Continue
    }
}

async fn pilot(rows: usize, cursor: CursorType, size: Size) -> Pilot {
    let app = TableApp {
        rows,
        cursor: Some(cursor),
        events: vec![],
    };
    App::new(app).test(size).await
}

fn events(p: &Pilot) -> Vec<String> {
    p.app::<TableApp>().unwrap().events.clone()
}

#[tokio::test]
async fn renders_header_and_rows() {
    let mut p = pilot(5, CursorType::Cell, Size::new(60, 10)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].starts_with(" Name  Num  Wide"), "{text}");
    assert!(lines[1].starts_with(" row0  0    w0"), "{text}");
    assert!(lines[5].starts_with(" row4  2    w4"), "{text}");
    // Mount posts the initial highlight.
    assert_eq!(events(&p), vec!["cell-hl 0 0"]);
}

#[tokio::test]
async fn cell_colors_cursor_zebra_header() {
    let mut p = pilot(5, CursorType::Cell, Size::new(60, 10)).await;
    let canvas = p.canvas();
    let header_bg = canvas.get(1, 0).unwrap().bg;
    let cursor_bg = canvas.get(1, 1).unwrap().bg;
    let row1_bg = canvas.get(1, 2).unwrap().bg;
    let row2_bg = canvas.get(1, 3).unwrap().bg;
    let row0_other = canvas.get(8, 1).unwrap().bg;
    assert_ne!(header_bg, row1_bg);
    assert_ne!(
        cursor_bg, row0_other,
        "cursor cell differs from rest of row"
    );
    assert_ne!(row1_bg, row2_bg, "zebra stripes");
    assert_eq!(row0_other, row2_bg, "even rows share background");
}

#[tokio::test]
async fn keyboard_navigation_and_select() {
    let mut p = pilot(5, CursorType::Cell, Size::new(60, 10)).await;
    p.press(&["down", "down", "right"]).await;
    let t = p.query_one::<DataTable>("#t").unwrap();
    assert_eq!(t.cursor_coordinate(), Coordinate::new(2, 1));
    p.press(&["enter"]).await;
    let ev = events(&p);
    assert_eq!(
        ev[1..],
        ["cell-hl 1 0", "cell-hl 2 0", "cell-hl 2 1", "cell-sel 1 k2"]
    );
    p.press(&["end"]).await;
    assert_eq!(p.query_one::<DataTable>("#t").unwrap().cursor_column(), 2);
    p.press(&["home", "ctrl+end"]).await;
    assert_eq!(
        p.query_one::<DataTable>("#t").unwrap().cursor_coordinate(),
        Coordinate::new(4, 0)
    );
    p.press(&["up", "up", "up", "up", "up", "up"]).await;
    assert_eq!(p.query_one::<DataTable>("#t").unwrap().cursor_row(), 0);
}

#[tokio::test]
async fn row_cursor_messages() {
    let mut p = pilot(5, CursorType::Row, Size::new(60, 10)).await;
    p.press(&["down", "enter"]).await;
    assert_eq!(events(&p), vec!["row-hl 0", "row-hl 1", "row-sel 1 k1"]);
    // Row cursor highlights the whole row (including the extended space).
    let canvas = p.canvas();
    let a = canvas.get(1, 2).unwrap().bg;
    let b = canvas.get(55, 2).unwrap().bg;
    assert_eq!(a, b);
    assert_ne!(a, canvas.get(1, 3).unwrap().bg);
}

#[tokio::test]
async fn column_cursor() {
    let mut p = pilot(3, CursorType::Column, Size::new(60, 10)).await;
    p.press(&["right", "enter"]).await;
    assert!(events(&p).contains(&"col-sel num".to_string()));
}

#[tokio::test]
async fn scrolling_keeps_cursor_visible_and_header_fixed() {
    let mut p = pilot(100, CursorType::Row, Size::new(60, 10)).await;
    for _ in 0..20 {
        p.press(&["down"]).await;
    }
    let text = p.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].contains("Name"), "header stays: {text}");
    assert!(text.contains("row20"), "{text}");
    assert!(!text.contains("row5 "), "{text}");
    // Cursor row is the last visible line.
    assert!(lines[9].contains("row20"), "{text}");
    p.press(&["pagedown"]).await;
    let row = p.query_one::<DataTable>("#t").unwrap().cursor_row();
    assert!(row > 25, "{row}");
    let text = p.screen_text();
    assert!(text.contains(&format!("row{row} ")), "{text}");
    assert!(text.lines().next().unwrap().contains("Name"));
    p.press(&["ctrl+home"]).await;
    assert!(p.screen_text().contains("row0 "));
}

#[tokio::test]
async fn mouse_wheel_scrolls() {
    let mut p = pilot(100, CursorType::Cell, Size::new(60, 10)).await;
    p.scroll_down_at(5, 5).await;
    let text = p.screen_text();
    assert!(text.lines().next().unwrap().contains("Name"), "{text}");
    assert!(!text.contains("row0 "), "{text}");
    p.scroll_up_at(5, 5).await;
    assert!(p.screen_text().contains("row0 "));
}

#[tokio::test]
async fn click_moves_cursor_and_selects() {
    let mut p = pilot(5, CursorType::Cell, Size::new(60, 10)).await;
    // Row 2 is on screen line 3; "Num" column spans x 6..10.
    p.click_at(7, 3).await;
    assert_eq!(
        p.query_one::<DataTable>("#t").unwrap().cursor_coordinate(),
        Coordinate::new(2, 1)
    );
    assert_eq!(p.focused_id().as_deref(), Some("t"));
    p.click_at(7, 3).await;
    assert!(
        events(&p).contains(&"cell-sel 1 k2".to_string()),
        "{:?}",
        events(&p)
    );
    // Header click.
    p.click_at(2, 0).await;
    assert!(events(&p).contains(&"header name".to_string()));
}

#[tokio::test]
async fn horizontal_scrolling_wide_table() {
    let mut p = pilot(5, CursorType::Cell, Size::new(20, 10)).await;
    assert!(p.screen_text().contains("Name"));
    p.press(&["right", "right"]).await;
    let text = p.screen_text();
    // The 32-cell "Wide" column is scrolled to its left edge.
    assert!(text.lines().next().unwrap().starts_with(" Wide"), "{text}");
    assert!(text.contains("w0"), "{text}");
    assert!(!text.contains("Name"), "{text}");
    p.press(&["home"]).await;
    assert!(p.screen_text().contains("Name"));
}

#[tokio::test]
async fn sort_update_remove_clear() {
    let mut p = pilot(5, CursorType::Cell, Size::new(60, 10)).await;
    p.with::<DataTable, _>("#t", |t, _ctx| {
        t.sort(&["num"], false).unwrap();
    })
    .await;
    let text = p.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    // nums: 0,7,1,8,2 -> sorted 0,1,2,7,8 -> row0,row2,row4,row1,row3
    assert!(
        lines[1].contains("row0") && lines[2].contains("row2"),
        "{text}"
    );
    assert!(lines[5].contains("row3"), "{text}");
    p.with::<DataTable, _>("#t", |t, _| {
        t.sort(&["name"], true).unwrap();
        t.update_cell("k4", "name", "[b]changed[/]", true).unwrap();
        assert_eq!(t.get_cell("k4", "name").unwrap().plain(), "changed");
        t.remove_row("k0").unwrap();
    })
    .await;
    let text = p.screen_text();
    assert!(text.lines().nth(1).unwrap().contains("changed"), "{text}");
    assert!(!text.contains("row0"));
    assert_eq!(p.query_one::<DataTable>("#t").unwrap().row_count(), 4);
    p.with::<DataTable, _>("#t", |t, _| t.clear(false)).await;
    let t = p.query_one::<DataTable>("#t").unwrap();
    assert_eq!(t.row_count(), 0);
    assert!(p.screen_text().contains("Name"));
}

#[tokio::test]
async fn move_cursor_scrolls() {
    let mut p = pilot(200, CursorType::Row, Size::new(60, 10)).await;
    p.with::<DataTable, _>("#t", |t, ctx| t.move_cursor(Some(150), None, ctx))
        .await;
    let text = p.screen_text();
    assert!(text.contains("row150"), "{text}");
    assert!(events(&p).contains(&"row-hl 150".to_string()));
}

#[tokio::test]
async fn row_labels_and_fixed_columns() {
    struct App2;
    impl Widget for App2 {
        fn compose(&mut self) -> Vec<Child> {
            let mut t = DataTable::new().fixed_columns(1);
            t.add_columns(&["A", "B", "C"]);
            for i in 0..3 {
                t.add_row_with(
                    [format!("a{i}"), format!("b{i}"), "c".repeat(40)],
                    Some(1),
                    None,
                    Some(&format!("L{i}")),
                )
                .unwrap();
            }
            compose![t.id("t")]
        }
    }
    let mut p = App::new(App2).test(Size::new(30, 6)).await;
    let text = p.screen_text();
    assert!(
        text.lines().nth(1).unwrap().starts_with(" L0  a0  b0"),
        "{text}"
    );
    p.press(&["right", "right"]).await;
    let text = p.screen_text();
    let line = text.lines().nth(1).unwrap();
    // Label and fixed column stay; B scrolled away.
    assert!(line.starts_with(" L0  a0  ccc"), "{text}");
}

#[tokio::test]
async fn large_table_renders_fast() {
    let start = Instant::now();
    let mut p = pilot(10_000, CursorType::Row, Size::new(80, 30)).await;
    p.press(&["ctrl+end"]).await;
    let t0 = Instant::now();
    for _ in 0..10 {
        p.press(&["up"]).await;
        let _ = p.screen_text();
    }
    let per_frame = t0.elapsed() / 10;
    assert!(p.screen_text().contains("row9989"), "{}", p.screen_text());
    assert!(
        per_frame.as_millis() < 100,
        "frame took {per_frame:?} (total {:?})",
        start.elapsed()
    );
}

#[tokio::test]
async fn hover_highlight() {
    let mut p = pilot(5, CursorType::Cell, Size::new(60, 10)).await;
    // The table is 6 lines tall; its center (30, 3) is in the "Wide" column of row 2.
    let before = p.canvas().get(30, 3).unwrap().bg;
    p.hover("#t").await;
    assert_eq!(
        p.query_one::<DataTable>("#t").unwrap().hover_coordinate(),
        (2, 2)
    );
    assert_ne!(p.canvas().get(30, 3).unwrap().bg, before);
    assert_eq!(
        p.canvas().get(30, 2).unwrap().bg,
        p.canvas().get(30, 4).unwrap().bg
    );
}

#[tokio::test]
async fn cell_alignment() {
    use rustual::render::Attrs;
    use rustual::widgets::data_table::Justify;
    struct A;
    impl Widget for A {
        fn compose(&mut self) -> Vec<Child> {
            let mut t = DataTable::new().cursor_type(CursorType::None);
            t.add_column_with("Item", Some(8), Some("item")).unwrap();
            t.add_column_with("Price", Some(8), Some("price")).unwrap();
            t.add_column_with("Tag", Some(8), Some("tag")).unwrap();
            t.add_row([
                CellValue::from("apple"),
                CellValue::right(1.5),
                CellValue::center("[b]new[/]"),
            ]);
            t.add_row([
                CellValue::from("pear"),
                CellValue::from(12).justify(Justify::Right),
                CellValue::from(7),
            ]);
            t.add_row([
                CellValue::from("fig"),
                CellValue::right(100.25),
                CellValue::from("x").justify(Justify::Left),
            ]);
            compose![t.id("t")]
        }
    }
    let mut p = App::new(A).test(Size::new(40, 6)).await;
    assert!(p.css_errors().is_empty());
    let text = p.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    // Column width 8 + cell padding 1 either side: content cells at x = 1, 11, 21.
    let cell = |row: usize, col: usize| -> String {
        let s: String = lines[row].chars().skip(col * 10 + 1).take(8).collect();
        format!("{s:<8}")
    };
    assert_eq!(cell(1, 0), "apple   ", "{text}");
    assert_eq!(cell(1, 1), "    1.50", "{text}");
    assert_eq!(cell(1, 2), "  new   ", "{text}");
    assert_eq!(cell(2, 1), "      12", "{text}");
    // Numbers are left-aligned unless asked otherwise (as in Textual).
    assert_eq!(cell(2, 2), "7       ", "{text}");
    assert_eq!(cell(3, 1), "  100.25", "{text}");

    // Markup styles survive alignment.
    assert!(p.canvas().get(23, 1).unwrap().attrs.contains(Attrs::BOLD));

    // Alignment is transparent for values, plain text and sorting.
    let t = p.query_one::<DataTable>("#t").unwrap();
    assert_eq!(
        t.get_cell_at(Coordinate::new(0, 1)).unwrap().plain(),
        "1.50"
    );
    assert_eq!(
        CellValue::right(2).compare(&CellValue::from(10)),
        std::cmp::Ordering::Less
    );
    assert_eq!(CellValue::right(2).inner(), &CellValue::Int(2));
    assert_eq!(CellValue::right(2).get_justify(), Justify::Right);
    assert_eq!(CellValue::from(2).get_justify(), Justify::Left);
    p.with::<DataTable, _>("#t", |t, _| t.sort(&["price"], true).unwrap())
        .await;
    let text = p.screen_text();
    assert!(text.lines().nth(1).unwrap().contains("fig"), "{text}");
}
