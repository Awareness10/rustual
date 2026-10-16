//! A live system monitor: CPU / memory / network cards with sparklines, a process table that
//! updates every second (sortable, with a details dialog and "kill" with confirmation), and a
//! layout that drops the cards on narrow terminals (breakpoints).
//!
//! The processes are simulated, so the example runs anywhere. Run with
//! `cargo run -p rustual --example monitor`. The footer lists the keys: `c` / `m` / `n` sort by
//! CPU / memory / name, `d` or enter shows details, `k` kills, space pauses, `ctrl+q` quits.

use rustual::prelude::*;
use rustual::widgets::data_table::RowSelected;
use std::time::Duration;

const CSS: &str = "
#cards { height: 9; }
.card { width: 1fr; border: round $primary; padding: 0 1; }
.card Digits { width: auto; }
.card Sparkline { height: 2; margin-top: 1; }
#procs { height: 1fr; border: round $secondary; }
Screen.-narrow #cards { display: none; }
";

const HISTORY: usize = 60;
const CORES: f64 = 8.0;
const MEMORY_MB: f64 = 16_384.0;

const NAMES: [&str; 16] = [
    "postgres",
    "nginx",
    "rust-analyzer",
    "cargo",
    "firefox",
    "code",
    "dockerd",
    "sshd",
    "redis-server",
    "python3",
    "node",
    "systemd",
    "pipewire",
    "Xwayland",
    "kworker",
    "rustual",
];

/// Deterministic pseudo-random numbers in `0.0..1.0` (xorshift), so runs are reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % 10_000) as f64 / 10_000.0
    }
}

#[derive(Debug, Clone)]
struct Process {
    pid: u32,
    name: &'static str,
    cpu: f64,
    memory: f64,
    threads: u32,
    history: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Cpu,
    Memory,
    Name,
}

impl SortKey {
    fn label(self) -> &'static str {
        match self {
            SortKey::Cpu => "CPU",
            SortKey::Memory => "memory",
            SortKey::Name => "name",
        }
    }
}

// --------------------------------------------------------------------------------- dialogs

/// Details of one process, with its CPU history.
struct Details {
    process: Process,
}

impl Widget for Details {
    fn type_names(&self) -> &'static [&'static str] {
        &["Details", "ModalScreen", "Screen"]
    }

    fn default_css(&self) -> &'static str {
        "
Details { align: center middle; }
#details { width: 60; height: auto; padding: 1 2; border: thick $accent; background: $surface; }
#details Sparkline { height: 4; margin-top: 1; }
"
    }

    fn compose(&mut self) -> Vec<Child> {
        let p = &self.process;
        let info = format!(
            "[b]{}[/] (pid {})\n\nCPU     [b]{:.1}%[/]\nMemory  [b]{:.0} MB[/]\nThreads {}\n\n[$text-muted]CPU over the last minute:[/]",
            p.name, p.pid, p.cpu, p.memory, p.threads
        );
        compose![Vertical::new().id("details").with(compose![
            Static::new(info),
            Sparkline::new(p.history.clone()),
        ])]
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("escape,enter", "app.pop_screen", "Close")]
    }
}

/// The answer of the kill confirmation.
struct KillConfirmed(bool);

struct ConfirmKill {
    name: String,
    pid: u32,
}

impl Widget for ConfirmKill {
    fn type_names(&self) -> &'static [&'static str] {
        &["ConfirmKill", "ModalScreen", "Screen"]
    }

    fn default_css(&self) -> &'static str {
        "
ConfirmKill { align: center middle; }
#confirm { width: 52; height: auto; padding: 1 2; border: thick $error; background: $surface; }
#confirm Horizontal { height: auto; align-horizontal: right; margin-top: 1; }
#confirm Button { margin-left: 2; }
"
    }

    fn compose(&mut self) -> Vec<Child> {
        compose![Vertical::new().id("confirm").with(compose![
            Label::new(format!(
                "Send SIGTERM to [b]{}[/] (pid {})?",
                self.name, self.pid
            )),
            Horizontal::new().with(compose![
                Button::new("Cancel").id("no"),
                Button::error("Kill").id("yes"),
            ]),
        ])]
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("y", "answer(True)", "Kill"),
            Binding::new("n,escape", "answer(False)", "Cancel"),
        ]
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        if action == "answer" {
            ctx.dismiss(KillConfirmed(args.first().is_some_and(|a| a == "True")));
            return true;
        }
        false
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if msg.get::<button::Pressed>().is_some() {
            ctx.dismiss(KillConfirmed(msg.sender_id() == Some("yes")));
            return Flow::Stop;
        }
        Flow::Continue
    }
}

// ------------------------------------------------------------------------------------- app

struct Monitor {
    rng: Rng,
    processes: Vec<Process>,
    cpu_history: Vec<f64>,
    memory_history: Vec<f64>,
    network_history: Vec<f64>,
    sort: SortKey,
    paused: bool,
    killing: Option<u32>,
}

impl Monitor {
    fn new() -> Self {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let processes = (0..40)
            .map(|i| {
                let cpu = rng.next() * 15.0;
                Process {
                    pid: 300 + i * 37,
                    name: NAMES[i as usize % NAMES.len()],
                    cpu,
                    memory: 20.0 + rng.next() * 380.0,
                    threads: 1 + (rng.next() * 40.0) as u32,
                    history: vec![cpu],
                }
            })
            .collect();
        let mut monitor = Monitor {
            rng,
            processes,
            cpu_history: Vec::new(),
            memory_history: Vec::new(),
            network_history: Vec::new(),
            sort: SortKey::Cpu,
            paused: false,
            killing: None,
        };
        for _ in 0..HISTORY / 2 {
            monitor.step();
        }
        monitor
    }

    /// Advance the simulation by one second.
    fn step(&mut self) {
        for p in &mut self.processes {
            // Mostly small changes, with an occasional spike.
            let spike = if self.rng.next() > 0.97 { 35.0 } else { 0.0 };
            let drift = (self.rng.next() - 0.52) * 6.0 + spike;
            p.cpu = (p.cpu * 0.9 + drift).clamp(0.0, 100.0);
            p.memory = (p.memory + (self.rng.next() - 0.5) * 12.0).clamp(5.0, 1024.0);
            p.history.push(p.cpu);
            if p.history.len() > HISTORY {
                p.history.remove(0);
            }
        }
        let push = |history: &mut Vec<f64>, value: f64| {
            history.push(value);
            if history.len() > HISTORY {
                history.remove(0);
            }
        };
        let cpu = (self.processes.iter().map(|p| p.cpu).sum::<f64>() / CORES).min(100.0);
        let memory = self.processes.iter().map(|p| p.memory).sum::<f64>();
        let network = 20.0 + self.rng.next() * 80.0;
        push(&mut self.cpu_history, cpu);
        push(&mut self.memory_history, memory);
        push(&mut self.network_history, network);
    }

    fn sorted(&self) -> Vec<&Process> {
        let mut procs: Vec<&Process> = self.processes.iter().collect();
        match self.sort {
            SortKey::Cpu => procs.sort_by(|a, b| b.cpu.total_cmp(&a.cpu)),
            SortKey::Memory => procs.sort_by(|a, b| b.memory.total_cmp(&a.memory)),
            SortKey::Name => procs.sort_by(|a, b| a.name.cmp(b.name).then(a.pid.cmp(&b.pid))),
        }
        procs
    }

    /// The pid of the row under the table cursor.
    fn current(&self, ctx: &mut Ctx) -> Option<u32> {
        let table = ctx.query_one::<DataTable>("#procs")?;
        let key = table.row_keys().get(table.cursor_row())?.0.clone();
        key.parse().ok()
    }

    fn refresh(&self, ctx: &mut Ctx) {
        let cpu = self.cpu_history.last().copied().unwrap_or_default();
        let memory = self.memory_history.last().copied().unwrap_or_default();
        let network = self.network_history.last().copied().unwrap_or_default();
        for (id, value, history) in [
            ("cpu", format!("{cpu:.0}%"), &self.cpu_history),
            (
                "memory",
                format!("{:.1}G", memory / 1024.0),
                &self.memory_history,
            ),
            ("network", format!("{network:.0}M"), &self.network_history),
        ] {
            ctx.with::<Digits, _>(&format!("#{id}-value"), |d, _| d.update(value));
            let data = history.clone();
            ctx.with::<Sparkline, _>(&format!("#{id}-spark"), |s, _| s.set_data(data));
        }
        ctx.with::<ProgressBar, _>("#memory-bar", |b, _| {
            b.update(Some(MEMORY_MB), memory.min(MEMORY_MB))
        });

        // Keep the cursor on the same process while the order changes.
        let selected = self.current(ctx);
        let rows: Vec<(String, Vec<CellValue>)> = self
            .sorted()
            .into_iter()
            .map(|p| {
                let cpu = if p.cpu > 50.0 {
                    format!("[b $error]{:.1}[/]", p.cpu)
                } else if p.cpu > 25.0 {
                    format!("[$warning]{:.1}[/]", p.cpu)
                } else {
                    format!("{:.1}", p.cpu)
                };
                (
                    p.pid.to_string(),
                    vec![
                        CellValue::right(p.pid as i64),
                        p.name.into(),
                        CellValue::right(cpu),
                        CellValue::right(format!("{:.0}", p.memory)),
                        CellValue::right(p.threads as i64),
                    ],
                )
            })
            .collect();
        ctx.with::<DataTable, _>("#procs", |table, ctx| {
            table.clear(false);
            let mut cursor = 0;
            for (i, (key, cells)) in rows.into_iter().enumerate() {
                if Some(key.as_str()) == selected.map(|p| p.to_string()).as_deref() {
                    cursor = i;
                }
                let _ = table.add_row_with(cells, Some(1), Some(&key), None);
            }
            table.move_cursor(Some(cursor), None, ctx);
        });
        let state = if self.paused { " · paused" } else { "" };
        ctx.set_sub_title(&format!(
            "{} processes · sorted by {}{state}",
            self.processes.len(),
            self.sort.label()
        ));
    }

    fn tick(&mut self, ctx: &mut Ctx) {
        if !self.paused {
            self.step();
            self.refresh(ctx);
        }
    }
}

fn card(id: &str, title: &str) -> Child {
    let mut children = compose![
        Digits::new("0").id(&format!("{id}-value")),
        Sparkline::new(Vec::<f64>::new()).id(&format!("{id}-spark")),
    ];
    if id == "memory" {
        children.push(
            ProgressBar::new(Some(MEMORY_MB))
                .show_eta(false)
                .id("memory-bar"),
        );
    }
    Vertical::new()
        .classes("card")
        .with(children)
        .border_title(title)
}

impl Widget for Monitor {
    fn compose(&mut self) -> Vec<Child> {
        let mut table = DataTable::new()
            .cursor_type(CursorType::Row)
            .zebra_stripes(true);
        for (label, key) in [
            ("PID", "pid"),
            ("Name", "name"),
            ("CPU %", "cpu"),
            ("Mem MB", "mem"),
            ("Threads", "threads"),
        ] {
            let _ = table.add_column_with(label, None, Some(key));
        }
        compose![
            Header::new().show_clock(true),
            Horizontal::new().id("cards").with(compose![
                card("cpu", "CPU"),
                card("memory", "Memory"),
                card("network", "Network"),
            ]),
            table.id("procs").border_title("Processes"),
            Footer::new(),
        ]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_title("Monitor");
        self.refresh(ctx);
        if let Some(table) = ctx.query_id("#procs") {
            ctx.focus(table);
        }
        ctx.set_interval(Duration::from_secs(1), |w: &mut Monitor, ctx| w.tick(ctx));
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("c", "sort('cpu')", "CPU"),
            Binding::new("m", "sort('memory')", "Memory"),
            Binding::new("n", "sort('name')", "Name"),
            // Enter also opens details (the table's own enter binding selects the row).
            Binding::new("d", "details", "Details"),
            Binding::new("k", "kill", "Kill"),
            Binding::new("space", "pause", "Pause"),
            Binding::new("ctrl+q", "quit", "Quit").priority(),
        ]
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "sort" => {
                self.sort = match args.first().map(String::as_str) {
                    Some("memory") => SortKey::Memory,
                    Some("name") => SortKey::Name,
                    _ => SortKey::Cpu,
                };
                self.refresh(ctx);
            }
            "details" => {
                if let Some(p) = self
                    .current(ctx)
                    .and_then(|pid| self.processes.iter().find(|p| p.pid == pid))
                {
                    ctx.push_modal(Details { process: p.clone() });
                }
            }
            "kill" => {
                if let Some(p) = self
                    .current(ctx)
                    .and_then(|pid| self.processes.iter().find(|p| p.pid == pid))
                {
                    self.killing = Some(p.pid);
                    ctx.push_modal(ConfirmKill {
                        name: p.name.to_string(),
                        pid: p.pid,
                    });
                }
            }
            "pause" => {
                self.paused = !self.paused;
                self.refresh(ctx);
            }
            _ => return false,
        }
        true
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(KillConfirmed(yes)) = msg.get::<KillConfirmed>() {
            if let (true, Some(pid)) = (*yes, self.killing.take()) {
                if let Some(i) = self.processes.iter().position(|p| p.pid == pid) {
                    let p = self.processes.remove(i);
                    ctx.notify(&format!("Sent SIGTERM to {} ({})", p.name, p.pid));
                }
                self.refresh(ctx);
            }
            return Flow::Stop;
        }
        if msg.is::<RowSelected>() {
            self.run_action("details", &[], ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }
}

fn app() -> App {
    App::new(Monitor::new())
        .css(CSS)
        .horizontal_breakpoints(&[(0, "-narrow"), (90, "-wide")])
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    app().run().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(p: &Pilot) -> Vec<String> {
        let t = p.query_one::<DataTable>("#procs").unwrap();
        (0..t.row_count())
            .map(|r| t.get_row_at(r).unwrap()[1].plain())
            .collect()
    }

    fn cpus(p: &Pilot) -> Vec<f64> {
        let t = p.query_one::<DataTable>("#procs").unwrap();
        (0..t.row_count())
            .map(|r| t.get_row_at(r).unwrap()[2].plain().trim().parse().unwrap())
            .collect()
    }

    #[tokio::test(start_paused = true)]
    async fn sorts_updates_pauses_and_kills() {
        let mut p = app().test(Size::new(110, 36)).await;
        assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
        let footer = p
            .screen_text()
            .lines()
            .last()
            .unwrap_or_default()
            .to_string();
        assert!(
            footer.contains("c CPU") && footer.contains("d Details") && footer.contains("^q Quit"),
            "{footer}"
        );

        // Sorted by CPU, highest first, and updated every second.
        let first = cpus(&p);
        assert!(first.windows(2).all(|w| w[0] >= w[1]), "{first:?}");
        p.wait(Duration::from_secs(1)).await;
        assert_ne!(cpus(&p), first);

        // Sort by name.
        p.press(&["n"]).await;
        let by_name = names(&p);
        let mut sorted = by_name.clone();
        sorted.sort();
        assert_eq!(by_name, sorted);

        // Paused: nothing changes.
        p.press(&["space"]).await;
        let frozen = cpus(&p);
        p.wait(Duration::from_secs(3)).await;
        assert_eq!(cpus(&p), frozen);
        assert!(p.screen_text().contains("paused"));

        // Details, then kill the selected process.
        p.press(&["enter"]).await;
        assert!(p.screen_text().contains("CPU over the last minute"));
        p.press(&["escape"]).await;
        p.press(&["k"]).await;
        p.press(&["y"]).await;
        assert_eq!(names(&p).len(), 39);
        assert!(p.notifications()[0].starts_with("Sent SIGTERM"));
    }

    #[tokio::test]
    async fn narrow_terminals_hide_the_cards() {
        let p = app().test(Size::new(70, 30)).await;
        let cards = p.query("#cards")[0];
        assert!(
            !p.dom().get(cards).unwrap().layout.visible
                || p.dom().get(cards).unwrap().layout.region.height == 0
        );
    }
}
