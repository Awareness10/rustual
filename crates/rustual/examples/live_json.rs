//! Live JSON form: JSON objects arriving over TCP are written into input fields in real time.
//!
//! The Rust twin of `examples/live_json/rustual_app.py` (same UI, options and wire format):
//!
//! ```text
//! cargo run -p rustual --example live_json                    # listen on 127.0.0.1:7878, feed itself at 60 Hz
//! cargo run -p rustual --example live_json -- --rate 240      # a faster internal feed
//! cargo run -p rustual --example live_json -- --no-feed       # only listen (feed with examples/live_json/feed.py)
//! cargo run -p rustual --example live_json -- --mode async    # receive with tokio instead of threads
//! cargo run -p rustual --example live_json -- --duration 10   # quit after 10 seconds and print the stats
//! ```
//!
//! Each line is a JSON object; values whose key matches an input's id are written into that
//! input, and `ts` (the sender's Unix time) gives the latency. The receiver runs off the app
//! thread (OS threads with blocking sockets, or a tokio worker) and hands every object to the UI
//! with [`AppHandle::post_message`], which never blocks. Focus a field to hold its value while
//! you edit it.

use rustual::prelude::*;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const FIELDS: [&str; 10] = [
    "vehicle",
    "status",
    "lat",
    "lon",
    "altitude",
    "speed",
    "heading",
    "battery",
    "temperature",
    "seq",
];

const CSS: &str = "
#main { align: center middle; }
#form {
    grid-size: 2;
    grid-columns: 13 1fr;
    grid-gutter: 0 1;
    width: 64;
    height: auto;
    border: round $primary;
    border-title-color: $text-primary;
    padding: 0 1;
}
#form Label { width: 100%; text-align: right; color: $text-muted; }
#form Input { border: none; height: 1; padding: 0 1; }
#form Input:focus { background: $primary 30%; }
#stats { width: 100%; height: 1; margin-top: 1; color: $text-muted; text-align: center; }
";

/// One JSON object from a client (`None` if the line wasn't a JSON object).
struct JsonReceived(Option<Map<String, Value>>);

/// A client connected (1) or disconnected (-1).
struct ClientsChanged(i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// An OS thread accepts clients; each client is read by a thread of its own.
    Thread,
    /// A tokio worker accepts and reads clients.
    Async,
}

#[derive(Debug, Clone)]
struct Options {
    host: String,
    /// 0: any free port.
    port: u16,
    mode: Mode,
    /// Rate of the internal feeder in Hz (`None`: wait for clients).
    feed_rate: Option<f64>,
    /// Quit after this many seconds.
    duration: Option<f64>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            host: "127.0.0.1".into(),
            port: 7878,
            mode: Mode::Thread,
            feed_rate: Some(60.0),
            duration: None,
        }
    }
}

fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

// ------------------------------------------------------------------------------------ stats

/// Receive rate and latency (the sender's `ts` to the moment the values are applied).
#[derive(Default)]
struct Stats {
    total: u64,
    errors: u64,
    latency_sum: f64,
    latency_max: f64,
    first: Option<Instant>,
    last: Option<Instant>,
    /// (time, latency ms) over the last second.
    recent: VecDeque<(Instant, f64)>,
}

impl Stats {
    fn record(&mut self, data: &Map<String, Value>) {
        let now = Instant::now();
        let latency = data
            .get("ts")
            .and_then(Value::as_f64)
            .map_or(0.0, |ts| ((unix_now() - ts) * 1000.0).max(0.0));
        self.total += 1;
        self.latency_sum += latency;
        self.latency_max = self.latency_max.max(latency);
        self.first.get_or_insert(now);
        self.last = Some(now);
        self.recent.push_back((now, latency));
        self.prune(now);
    }

    fn prune(&mut self, now: Instant) {
        while self
            .recent
            .front()
            .is_some_and(|(t, _)| now.duration_since(*t) > Duration::from_secs(1))
        {
            self.recent.pop_front();
        }
    }

    fn line(&mut self, clients: i32, paused: bool) -> String {
        self.prune(Instant::now());
        let rate = self.recent.len();
        let avg = if rate > 0 {
            self.recent.iter().map(|(_, l)| l).sum::<f64>() / rate as f64
        } else {
            0.0
        };
        let peak = self.recent.iter().map(|(_, l)| *l).fold(0.0, f64::max);
        let plural = if clients != 1 { "s" } else { "" };
        let text = format!(
            "{rate:3} Hz · latency {avg:4.1} ms (max {peak:4.1}) · {} received · {} bad · {clients} client{plural}",
            self.total, self.errors
        );
        if paused {
            text + " · PAUSED"
        } else {
            text
        }
    }

    fn summary(&self) -> String {
        let span = match (self.first, self.last) {
            (Some(a), Some(b)) => b.duration_since(a).as_secs_f64(),
            _ => 0.0,
        };
        let rate = if span > 0.0 {
            (self.total - 1) as f64 / span
        } else {
            0.0
        };
        let avg = if self.total > 0 {
            self.latency_sum / self.total as f64
        } else {
            0.0
        };
        format!(
            "received {} objects ({} bad) at {rate:.1} Hz; latency avg {avg:.2} ms, max {:.2} ms",
            self.total, self.errors, self.latency_max
        )
    }
}

// ---------------------------------------------------------------------------- receiving

/// Parse one line into a message for the UI.
fn parse(line: &str) -> JsonReceived {
    JsonReceived(match serde_json::from_str::<Value>(line) {
        Ok(Value::Object(map)) => Some(map),
        _ => None,
    })
}

/// Accept clients on an OS thread until `stop` is set (then a connection wakes `accept`).
fn serve_threads(listener: TcpListener, handle: AppHandle, root: NodeId, stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            if stop.load(Ordering::Relaxed) || !handle.is_running() {
                break;
            }
            let Ok(conn) = conn else { continue };
            let handle = handle.clone();
            std::thread::spawn(move || {
                handle.post_message(root, ClientsChanged(1));
                for line in BufReader::new(conn).lines() {
                    let Ok(line) = line else { break };
                    // `false`: the app has exited.
                    if !handle.post_message(root, parse(&line)) {
                        return;
                    }
                }
                handle.post_message(root, ClientsChanged(-1));
            });
        }
    });
}

/// Accept and read clients in a tokio worker (cancelled with the app).
async fn serve_async(listener: TcpListener, handle: AppHandle, root: NodeId) {
    use tokio::io::AsyncBufReadExt;
    let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
        return;
    };
    // Dropping the set (when the worker is cancelled) aborts the client tasks too.
    let mut clients = tokio::task::JoinSet::new();
    while let Ok((conn, _)) = listener.accept().await {
        let handle = handle.clone();
        clients.spawn(async move {
            handle.post_message(root, ClientsChanged(1));
            let mut lines = tokio::io::BufReader::new(conn).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !handle.post_message(root, parse(&line)) {
                    return;
                }
            }
            handle.post_message(root, ClientsChanged(-1));
        });
    }
}

// ------------------------------------------------------------------------------ feeding

const STATUSES: [&str; 6] = [
    "nominal",
    "nominal",
    "nominal",
    "climbing",
    "descending",
    "low signal",
];

fn round(x: f64, digits: i32) -> f64 {
    let f = 10f64.powi(digits);
    (x * f).round() / f
}

/// One telemetry sample at `t` seconds since the feed started (as in `feed.py`).
fn sample(seq: u64, t: f64) -> Value {
    json!({
        "seq": seq,
        "ts": unix_now(),
        "vehicle": "rover-7",
        "lat": round(59.9139 + 0.01 * (t / 7.0).sin(), 6),
        "lon": round(10.7522 + 0.01 * (t / 7.0).cos(), 6),
        "altitude": round(120.0 + 30.0 * (t / 3.0).sin(), 2),
        "speed": round(12.0 + 4.0 * (t * 1.3).sin(), 2),
        "heading": round((t * 24.0) % 360.0, 1),
        "battery": round(100.0 - (t / 6.0) % 100.0, 1),
        "temperature": round(21.0 + 3.0 * (t / 11.0).sin(), 2),
        "status": STATUSES[(t / 2.0) as usize % STATUSES.len()],
    })
}

/// Send `rate` samples a second to `addr` on a fixed schedule until `stop` is set.
fn start_feeder(addr: SocketAddr, rate: f64, stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let Ok(mut sock) = TcpStream::connect_timeout(&addr, Duration::from_secs(5)) else {
            return;
        };
        let _ = sock.set_nodelay(true);
        let period = Duration::from_secs_f64(1.0 / rate);
        let start = Instant::now();
        let mut next = start;
        let mut seq = 0;
        while !stop.load(Ordering::Relaxed) {
            seq += 1;
            let line = format!("{}\n", sample(seq, start.elapsed().as_secs_f64()));
            if sock.write_all(line.as_bytes()).is_err() {
                break;
            }
            next += period;
            match next.checked_duration_since(Instant::now()) {
                Some(delay) => std::thread::sleep(delay),
                None => next = Instant::now(), // fell behind: don't burst to catch up
            }
        }
    });
}

// ------------------------------------------------------------------------------- the app

struct LiveForm {
    opts: Options,
    stats: Stats,
    clients: i32,
    paused: bool,
    inputs: HashMap<String, NodeId>,
    bound_port: Option<u16>,
    stop: Arc<AtomicBool>,
    /// The summary, set when `--duration` ends the app.
    summary: Arc<Mutex<Option<String>>>,
}

impl LiveForm {
    fn new(opts: Options) -> Self {
        LiveForm {
            opts,
            stats: Stats::default(),
            clients: 0,
            paused: false,
            inputs: HashMap::new(),
            bound_port: None,
            stop: Arc::new(AtomicBool::new(false)),
            summary: Arc::new(Mutex::new(None)),
        }
    }

    fn listen(&mut self, ctx: &mut Ctx) -> std::io::Result<()> {
        // Bind on the app thread so a busy port fails at once and port 0 is known right away.
        let listener = TcpListener::bind((self.opts.host.as_str(), self.opts.port))?;
        let addr = listener.local_addr()?;
        self.bound_port = Some(addr.port());
        let mode = match self.opts.mode {
            Mode::Thread => "thread",
            Mode::Async => "async",
        };
        if let Some(form) = ctx.query_id("#form") {
            let title = format!("{mode} receiver · {}:{}", self.opts.host, addr.port());
            ctx.for_node(form, |c| c.set_border_title(Some(&title)));
        }
        let handle = ctx.app_handle();
        let root = ctx.root();
        match self.opts.mode {
            Mode::Thread => serve_threads(listener, handle, root, self.stop.clone()),
            Mode::Async => {
                listener.set_nonblocking(true)?;
                ctx.run_worker(serve_async(listener, handle, root));
            }
        }
        if let Some(rate) = self.opts.feed_rate {
            ctx.set_sub_title(&format!("fed at {rate} Hz"));
            start_feeder(addr, rate, self.stop.clone());
        }
        Ok(())
    }

    fn apply(&mut self, data: &Map<String, Value>, ctx: &mut Ctx) {
        self.stats.record(data);
        if self.paused {
            return;
        }
        let focused = ctx.focused();
        for (key, value) in data {
            let Some(&id) = self.inputs.get(key) else {
                continue;
            };
            if focused == Some(id) {
                continue; // a focused field holds its value
            }
            let text = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            ctx.with_id::<Input, _>(id, |input, ctx| input.set_value(text, ctx));
        }
    }

    fn show_stats(&mut self, ctx: &mut Ctx) {
        let line = self.stats.line(self.clients, self.paused);
        if let Some(stats) = ctx.query_one::<Static>("#stats") {
            stats.update_plain(line);
        }
    }

    fn finish(&mut self, ctx: &mut Ctx) {
        *self.summary.lock().unwrap() = Some(self.stats.summary());
        ctx.exit();
    }
}

impl Widget for LiveForm {
    fn compose(&mut self) -> Vec<Child> {
        let mut form = Vec::new();
        for name in FIELDS {
            form.push(Label::new(name).child());
            form.push(Input::new().id(name));
        }
        compose![
            Header::new(),
            Vertical::new().id("main").with(compose![
                Center::new().with(compose![Grid::new().id("form").with(form)]),
                Static::new("").id("stats"),
            ]),
            Footer::new(),
        ]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        for name in FIELDS {
            if let Some(id) = ctx.query_id(&format!("#{name}")) {
                self.inputs.insert(name.to_string(), id);
            }
        }
        if let Err(e) = self.listen(ctx) {
            *self.summary.lock().unwrap() = Some(format!("can't listen: {e}"));
            ctx.exit();
            return;
        }
        ctx.set_interval(Duration::from_millis(250), |w: &mut LiveForm, ctx| {
            w.show_stats(ctx)
        });
        self.show_stats(ctx);
        if let Some(secs) = self.opts.duration {
            ctx.set_timer(Duration::from_secs_f64(secs), |w: &mut LiveForm, ctx| {
                w.finish(ctx)
            });
        }
    }

    fn on_unmount(&mut self, _ctx: &mut Ctx) {
        self.stop.store(true, Ordering::Relaxed);
        // Wake the accepting thread so it sees `stop`.
        if let (Mode::Thread, Some(port)) = (self.opts.mode, self.bound_port) {
            let _ = TcpStream::connect((self.opts.host.as_str(), port));
        }
    }

    fn on_message(&mut self, m: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(JsonReceived(data)) = m.get::<JsonReceived>() {
            match data {
                Some(data) => self.apply(data, ctx),
                None => self.stats.errors += 1,
            }
            return Flow::Stop;
        }
        if let Some(ClientsChanged(delta)) = m.get::<ClientsChanged>() {
            self.clients += delta;
            self.show_stats(ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("f2", "toggle_pause", "Pause"),
            Binding::new("f3", "reset_stats", "Reset stats"),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "toggle_pause" => self.paused = !self.paused,
            "reset_stats" => self.stats = Stats::default(),
            _ => return false,
        }
        self.show_stats(ctx);
        true
    }
}

fn app(opts: Options) -> (App, Arc<Mutex<Option<String>>>) {
    let form = LiveForm::new(opts);
    let summary = form.summary.clone();
    let app = App::new(form)
        .title("Live JSON form")
        .css(CSS)
        // A focused field holds its value, so start with none focused.
        .auto_focus(None);
    (app, summary)
}

fn parse_args() -> Result<Options, String> {
    let mut opts = Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--host" => opts.host = value("--host")?,
            "--port" => opts.port = value("--port")?.parse().map_err(|e| format!("--port: {e}"))?,
            "--mode" => {
                opts.mode = match value("--mode")?.as_str() {
                    "thread" => Mode::Thread,
                    "async" => Mode::Async,
                    other => return Err(format!("--mode: expected thread or async, not {other}")),
                }
            }
            "--rate" => {
                let rate: f64 = value("--rate")?.parse().map_err(|e| format!("--rate: {e}"))?;
                opts.feed_rate = Some(rate).filter(|r| *r > 0.0);
            }
            "--no-feed" => opts.feed_rate = None,
            "--duration" => {
                opts.duration = Some(
                    value("--duration")?
                        .parse()
                        .map_err(|e| format!("--duration: {e}"))?,
                )
            }
            "-h" | "--help" => {
                return Err("options: --host H --port N --mode thread|async --rate HZ --no-feed --duration SECS".into())
            }
            other => return Err(format!("unknown option {other} (see --help)")),
        }
    }
    Ok(opts)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let opts = match parse_args() {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let (app, summary) = app(opts);
    app.run().await?;
    if let Some(summary) = summary.lock().unwrap().take() {
        println!("{summary}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(mode: Mode) -> Options {
        Options {
            port: 0,
            mode,
            feed_rate: None,
            ..Options::default()
        }
    }

    fn value(p: &Pilot, id: &str) -> String {
        p.query_one::<Input>(&format!("#{id}"))
            .unwrap()
            .get_value()
            .to_string()
    }

    /// Wait (handling messages as they arrive) until `done` or a timeout.
    async fn wait_until(p: &mut Pilot, done: impl Fn(&LiveForm) -> bool) {
        for _ in 0..200 {
            if done(p.app::<LiveForm>().unwrap()) {
                return;
            }
            p.wait(Duration::from_millis(10)).await;
        }
        panic!("timed out");
    }

    async fn receives_lines(mode: Mode) {
        let (app, _) = app(opts(mode));
        let mut p = app.test(Size::new(80, 24)).await;
        assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
        assert_eq!(p.focused(), None, "auto_focus(None): nothing focused");
        let port = p.app::<LiveForm>().unwrap().bound_port.unwrap();
        let mut sock = TcpStream::connect(("127.0.0.1", port)).unwrap();
        writeln!(sock, "{}", sample(1, 0.0)).unwrap();
        writeln!(sock, "not json").unwrap();
        writeln!(sock, r#"{{"speed": 99.5, "status": "racing", "extra": 1}}"#).unwrap();
        sock.flush().unwrap();
        wait_until(&mut p, |f| f.stats.total == 2 && f.stats.errors == 1).await;
        assert_eq!(value(&p, "vehicle"), "rover-7");
        assert_eq!(value(&p, "seq"), "1");
        assert_eq!(value(&p, "speed"), "99.5");
        assert_eq!(value(&p, "status"), "racing");
        assert_eq!(p.app::<LiveForm>().unwrap().clients, 1);
        drop(sock);
        wait_until(&mut p, |f| f.clients == 0).await;
        p.wait(Duration::from_millis(300)).await;
        let text = p.screen_text();
        assert!(text.contains("2 received · 1 bad · 0 clients"), "{text}");
        let mode = if mode == Mode::Thread {
            "thread"
        } else {
            "async"
        };
        assert!(
            text.contains(&format!("{mode} receiver · 127.0.0.1:{port}")),
            "{text}"
        );
    }

    #[tokio::test]
    async fn thread_receiver_writes_json_into_inputs() {
        receives_lines(Mode::Thread).await;
    }

    #[tokio::test]
    async fn async_receiver_writes_json_into_inputs() {
        receives_lines(Mode::Async).await;
    }

    #[tokio::test]
    async fn pause_and_focus_hold_values() {
        let (app, _) = app(opts(Mode::Thread));
        let mut p = app.test(Size::new(80, 24)).await;
        let port = p.app::<LiveForm>().unwrap().bound_port.unwrap();
        let mut sock = TcpStream::connect(("127.0.0.1", port)).unwrap();
        writeln!(sock, r#"{{"speed": 1, "heading": 1}}"#).unwrap();
        wait_until(&mut p, |f| f.stats.total == 1).await;
        // A focused field keeps its value.
        p.click("#speed").await;
        writeln!(sock, r#"{{"speed": 2, "heading": 2}}"#).unwrap();
        wait_until(&mut p, |f| f.stats.total == 2).await;
        assert_eq!(
            (value(&p, "speed"), value(&p, "heading")),
            ("1".into(), "2".into())
        );
        // Paused: nothing is written, but the stats still count.
        p.press(&["f2"]).await;
        writeln!(sock, r#"{{"heading": 3}}"#).unwrap();
        wait_until(&mut p, |f| f.stats.total == 3).await;
        assert_eq!(value(&p, "heading"), "2");
        assert!(p.screen_text().contains("PAUSED"));
    }

    #[tokio::test]
    async fn feeds_itself() {
        let (app, _) = app(Options {
            feed_rate: Some(200.0),
            ..opts(Mode::Async)
        });
        let mut p = app.test(Size::new(80, 24)).await;
        wait_until(&mut p, |f| f.stats.total >= 20).await;
        let seq: u64 = value(&p, "seq").parse().unwrap();
        assert!(seq >= 20, "{seq}");
        assert_eq!(value(&p, "vehicle"), "rover-7");
    }

    #[tokio::test]
    async fn duration_ends_the_app_with_a_summary() {
        let (app, summary) = app(Options {
            duration: Some(0.05),
            ..opts(Mode::Thread)
        });
        let mut p = app.test(Size::new(80, 24)).await;
        p.wait(Duration::from_millis(100)).await;
        assert!(p.has_exited());
        let summary = summary.lock().unwrap().clone().unwrap();
        assert!(summary.starts_with("received 0 objects"), "{summary}");
    }
}
