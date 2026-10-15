//! A DataTable of generated data with a ListView sidebar and a Log of selection events.
//! Run with `cargo run -p rustual --example table`.
//!
//! Keys: arrows / pageup / pagedown move the row cursor, enter selects, `s` sorts by the
//! current column (again to reverse), click a header to sort by it, tab switches focus,
//! ctrl+q quits. Selecting a region in the sidebar filters the table.

use rustual::prelude::*;
use rustual::widgets::data_table::{HeaderSelected, RowHighlighted, RowSelected};
use rustual::widgets::list_view;

const CSS: &str = "
Screen { layout: horizontal; }
#sidebar { width: 22; height: 1fr; border: round $accent; }
#main { width: 1fr; height: 1fr; layout: vertical; }
#table { height: 1fr; border: round $primary; }
#log { height: 8; border: round $secondary; }
";

const REGIONS: [&str; 5] = ["All", "North", "South", "East", "West"];
const NAMES: [&str; 8] = [
    "Ada", "Grace", "Linus", "Guido", "Ken", "Dennis", "Barbara", "Margaret",
];

/// Deterministic pseudo-random numbers (xorshift).
fn rng(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

struct Record {
    id: usize,
    name: String,
    region: &'static str,
    score: i64,
    ratio: f64,
}

fn generate() -> Vec<Record> {
    let mut seed = 0x2545_f491_4f6c_dd1d;
    (0..200)
        .map(|id| Record {
            id,
            name: format!("{} {}", NAMES[(rng(&mut seed) % 8) as usize], id),
            region: REGIONS[1 + (rng(&mut seed) % 4) as usize],
            score: (rng(&mut seed) % 1000) as i64,
            ratio: (rng(&mut seed) % 10_000) as f64 / 100.0,
        })
        .collect()
}

struct TableApp {
    records: Vec<Record>,
    sort: Option<(String, bool)>,
}

impl TableApp {
    fn fill(&self, table: &mut DataTable, region: &str) {
        table.clear(false);
        for r in self
            .records
            .iter()
            .filter(|r| region == "All" || r.region == region)
        {
            let score = if r.score > 900 {
                format!("[b $success]{}[/]", r.score)
            } else {
                r.score.to_string()
            };
            table
                .add_row_with(
                    [
                        CellValue::from(r.id),
                        r.name.as_str().into(),
                        r.region.into(),
                        score.into(),
                        r.ratio.into(),
                    ],
                    Some(1),
                    Some(&format!("r{}", r.id)),
                    None,
                )
                .expect("unique keys");
        }
    }

    fn sort_by(&mut self, column: &str, ctx: &mut Ctx) {
        let reverse = matches!(&self.sort, Some((c, rev)) if c == column && !rev);
        self.sort = Some((column.to_string(), reverse));
        let col = column.to_string();
        ctx.with::<DataTable, _>("#table", |t, _| {
            let _ = t.sort(&[&col], reverse);
        });
        ctx.with::<Log, _>("#log", |log, ctx| {
            log.write_line(&format!("sorted by {col} (reverse={reverse})"), ctx)
        });
    }
}

impl Widget for TableApp {
    fn compose(&mut self) -> Vec<Child> {
        let mut table = DataTable::new()
            .zebra_stripes(true)
            .cursor_type(CursorType::Row);
        for (label, key) in [
            ("ID", "id"),
            ("Name", "name"),
            ("Region", "region"),
            ("Score", "score"),
            ("Ratio %", "ratio"),
        ] {
            table.add_column_with(label, None, Some(key)).unwrap();
        }
        self.fill(&mut table, "All");
        let items: Vec<Child> = REGIONS
            .iter()
            .map(|r| ListItem::label(*r).id(&r.to_lowercase()))
            .collect();
        compose![
            ListView::new()
                .with(items)
                .id("sidebar")
                .border_title("Regions"),
            rustual::widgets::Vertical::new().id("main").with(compose![
                table.id("table").border_title("Data"),
                Log::new().id("log").border_title("Events"),
            ]),
        ]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        if let Some(id) = ctx.query_id("#table") {
            ctx.focus(id);
        }
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("s", "sort", "Sort column"),
            Binding::new("q", "quit", "Quit"),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "sort" {
            let key = ctx
                .query_one::<DataTable>("#table")
                .map(|t| {
                    let c = t.cursor_column();
                    t.ordered_columns()[c].key.value().to_string()
                })
                .unwrap_or_else(|| "id".into());
            self.sort_by(&key, ctx);
            return true;
        }
        false
    }

    fn on_message(&mut self, m: &Message, ctx: &mut Ctx) -> Flow {
        let mut line = None;
        if let Some(h) = m.get::<HeaderSelected>() {
            let key = h.column_key.value().to_string();
            self.sort_by(&key, ctx);
        } else if let Some(r) = m.get::<RowHighlighted>() {
            line = Some(format!(
                "highlighted row {} ({})",
                r.cursor_row,
                r.row_key.value()
            ));
        } else if let Some(r) = m.get::<RowSelected>() {
            let name = ctx
                .query_one::<DataTable>("#table")
                .and_then(|t| t.get_cell(&r.row_key, "name").ok().map(|c| c.plain()))
                .unwrap_or_default();
            line = Some(format!("selected {name}"));
        } else if let Some(s) = m.get::<list_view::Selected>() {
            let region = REGIONS[s.index];
            if let Some(table) = ctx.query_one::<DataTable>("#table") {
                self.fill(table, region);
            }
            if let Some((col, rev)) = self.sort.clone() {
                ctx.with::<DataTable, _>("#table", |t, _| {
                    let _ = t.sort(&[&col], rev);
                });
            }
            ctx.with::<DataTable, _>("#table", |t, ctx| t.move_cursor(Some(0), None, ctx));
            line = Some(format!("region: {region}"));
        }
        if let Some(line) = line {
            ctx.with::<Log, _>("#log", |log, ctx| log.write_line(&line, ctx));
        }
        Flow::Continue
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    App::new(TableApp {
        records: generate(),
        sort: None,
    })
    .css(CSS)
    .title("DataTable demo")
    .run()
    .await
}
