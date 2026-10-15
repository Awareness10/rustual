//! Workers with state tracking (port of the parts of `textual/worker.py` and
//! `worker_manager.py` that fit an async-message design).
//!
//! [`Ctx::run_worker_with`] / [`Ctx::run_blocking_with`] run work in the background, post the
//! output to the widget as a message (like [`Ctx::run_worker`]) and additionally post
//! [`StateChanged`] messages as the worker moves through [`WorkerState`]s:
//! `Pending` (when created) → `Running` → `Success` | `Error` (panicked) | `Cancelled`.
//!
//! ```
//! # use rustual::prelude::*;
//! # async fn fetch() -> String { String::new() }
//! # fn demo(ctx: &mut Ctx, msg: &Message) {
//! ctx.run_worker_with(async { fetch().await }, WorkerOptions::new().name("fetch").group("net").exclusive());
//! // in on_message:
//! if let Some(e) = msg.get::<worker::StateChanged>() {
//!     println!("worker is now {:?}", e.state);
//! }
//! # }
//! ```
//!
//! Deviations from Textual: a worker error does not exit the app (Textual's `exit_on_error`);
//! the plain `run_worker` / `run_blocking` post no state messages (only their output).

use crate::app::AppHandle;
use crate::context::Ctx;
use crate::dom::NodeId;
use crate::message::Message;
use futures::FutureExt;
use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};

/// Identifies a worker created by [`Ctx::run_worker_with`] (or the plain variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkerId(pub(crate) u64);

/// Lifecycle of a worker (Textual: `WorkerState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WorkerState {
    /// Created, not yet started.
    #[default]
    Pending,
    Running,
    /// Finished; its output has been posted.
    Success,
    /// The worker panicked.
    Error,
    /// Cancelled before it finished.
    Cancelled,
}

impl WorkerState {
    /// `Success`, `Error` or `Cancelled`.
    pub fn is_finished(self) -> bool {
        matches!(
            self,
            WorkerState::Success | WorkerState::Error | WorkerState::Cancelled
        )
    }
}

/// Posted to the worker's owner when its state changes (Textual: `Worker.StateChanged`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateChanged {
    pub worker: WorkerId,
    pub state: WorkerState,
    pub name: String,
    pub group: String,
}

/// Options for [`Ctx::run_worker_with`] (Textual: `run_worker(name=, group=, exclusive=)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerOptions {
    pub name: String,
    /// Default `"default"`.
    pub group: String,
    /// Cancel this node's other running workers in the same group first.
    pub exclusive: bool,
}

impl Default for WorkerOptions {
    fn default() -> Self {
        WorkerOptions {
            name: String::new(),
            group: "default".to_string(),
            exclusive: false,
        }
    }
}

impl WorkerOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn group(mut self, group: &str) -> Self {
        self.group = group.to_string();
        self
    }

    pub fn exclusive(mut self) -> Self {
        self.exclusive = true;
        self
    }
}

/// Handle to a running worker; dropping it does not cancel the worker.
pub struct WorkerHandle {
    pub(crate) join: tokio::task::JoinHandle<()>,
    pub(crate) id: WorkerId,
    pub(crate) state: Arc<Mutex<WorkerState>>,
}

impl WorkerHandle {
    /// Cancel the worker (its owner gets `StateChanged { state: Cancelled }` for workers started
    /// with [`Ctx::run_worker_with`]). A blocking closure keeps running but its output is dropped.
    pub fn cancel(&self) {
        self.join.abort();
    }

    pub fn is_finished(&self) -> bool {
        self.join.is_finished()
    }

    pub fn id(&self) -> WorkerId {
        self.id
    }

    /// The worker's current state.
    pub fn state(&self) -> WorkerState {
        self.state.lock().map(|s| *s).unwrap_or_default()
    }
}

/// Registry of workers (per app), for `exclusive` and group cancellation.
#[derive(Default)]
pub(crate) struct Workers {
    next_id: u64,
    entries: Vec<Entry>,
}

struct Entry {
    node: NodeId,
    group: String,
    abort: tokio::task::AbortHandle,
}

impl Workers {
    fn prune(&mut self) {
        self.entries.retain(|e| !e.abort.is_finished());
    }

    /// Cancel `node`'s unfinished workers in `group` (all groups if `None`). Returns how many.
    pub(crate) fn cancel(&mut self, node: NodeId, group: Option<&str>) -> usize {
        self.prune();
        let mut n = 0;
        for e in &self.entries {
            if e.node == node && group.is_none_or(|g| g == e.group) {
                e.abort.abort();
                n += 1;
            }
        }
        n
    }

    /// Number of unfinished workers.
    pub(crate) fn running(&mut self) -> usize {
        self.prune();
        self.entries.len()
    }
}

/// Posts state changes; posts `Cancelled` if dropped before finishing (task aborted).
struct Reporter {
    handle: AppHandle,
    target: NodeId,
    id: WorkerId,
    name: String,
    group: String,
    state: Arc<Mutex<WorkerState>>,
    post: bool,
    finished: bool,
}

impl Reporter {
    fn set(&mut self, state: WorkerState) {
        if let Ok(mut s) = self.state.lock() {
            *s = state;
        }
        if state.is_finished() {
            self.finished = true;
        }
        if self.post {
            self.handle.post_message(
                self.target,
                StateChanged {
                    worker: self.id,
                    state,
                    name: self.name.clone(),
                    group: self.group.clone(),
                },
            );
        }
    }
}

impl Drop for Reporter {
    fn drop(&mut self) {
        if !self.finished {
            self.set(WorkerState::Cancelled);
        }
    }
}

enum Work<T> {
    Async(std::pin::Pin<Box<dyn Future<Output = T> + Send>>),
    Blocking(Box<dyn FnOnce() -> T + Send>),
}

fn spawn<T: Any + Send>(
    ctx: &mut Ctx,
    work: Work<T>,
    opts: WorkerOptions,
    post: bool,
) -> WorkerHandle {
    let app = &mut *ctx.app;
    let target = ctx.node;
    if opts.exclusive {
        app.workers.cancel(target, Some(&opts.group));
    }
    app.workers.prune();
    app.workers.next_id += 1;
    let id = WorkerId(app.workers.next_id);
    let state = Arc::new(Mutex::new(WorkerState::Pending));
    if post {
        let css = app.dom.get(target).and_then(|n| n.css_id.clone());
        let msg = Message::new(
            target,
            css,
            StateChanged {
                worker: id,
                state: WorkerState::Pending,
                name: opts.name.clone(),
                group: opts.group.clone(),
            },
        );
        app.post(target, msg);
    }
    let mut reporter = Reporter {
        handle: app.handle(),
        target,
        id,
        name: opts.name,
        group: opts.group.clone(),
        state: state.clone(),
        post,
        finished: false,
    };
    let join = tokio::spawn(async move {
        reporter.set(WorkerState::Running);
        let result = match work {
            Work::Async(fut) => AssertUnwindSafe(fut).catch_unwind().await.ok(),
            Work::Blocking(f) => tokio::task::spawn_blocking(f).await.ok(),
        };
        match result {
            Some(out) => {
                reporter.handle.post_message(target, out);
                reporter.set(WorkerState::Success);
            }
            None => reporter.set(WorkerState::Error),
        }
    });
    app.workers.entries.push(Entry {
        node: target,
        group: opts.group,
        abort: join.abort_handle(),
    });
    WorkerHandle { join, id, state }
}

impl Ctx<'_> {
    /// Run a future as a worker: its output is posted to this widget as a message, and
    /// [`StateChanged`] messages report its state (Textual: `run_worker(..., name, group,
    /// exclusive)`).
    pub fn run_worker_with<F, T>(&mut self, fut: F, opts: WorkerOptions) -> WorkerHandle
    where
        F: Future<Output = T> + Send + 'static,
        T: Any + Send,
    {
        spawn(self, Work::Async(Box::pin(fut)), opts, true)
    }

    /// Like [`Ctx::run_worker_with`] for a blocking closure (Textual: `thread=True`).
    pub fn run_blocking_with<F, T>(&mut self, f: F, opts: WorkerOptions) -> WorkerHandle
    where
        F: FnOnce() -> T + Send + 'static,
        T: Any + Send,
    {
        spawn(self, Work::Blocking(Box::new(f)), opts, true)
    }

    /// Cancel this widget's workers in `group` (Textual: `workers.cancel_group(self, group)`).
    /// Returns how many were cancelled.
    pub fn cancel_workers(&mut self, group: &str) -> usize {
        let node = self.node;
        self.app.workers.cancel(node, Some(group))
    }

    /// Cancel all of this widget's workers (Textual: `workers.cancel_node(self)`).
    pub fn cancel_all_workers(&mut self) -> usize {
        let node = self.node;
        self.app.workers.cancel(node, None)
    }

    /// Number of unfinished workers in the app.
    pub fn running_workers(&mut self) -> usize {
        self.app.workers.running()
    }
}

/// Plain workers (no state messages), used by [`Ctx::run_worker`] / [`Ctx::run_blocking`].
pub(crate) fn run_plain<F, T>(ctx: &mut Ctx, fut: F) -> WorkerHandle
where
    F: Future<Output = T> + Send + 'static,
    T: Any + Send,
{
    spawn(
        ctx,
        Work::Async(Box::pin(fut)),
        WorkerOptions::default(),
        false,
    )
}

pub(crate) fn run_plain_blocking<F, T>(ctx: &mut Ctx, f: F) -> WorkerHandle
where
    F: FnOnce() -> T + Send + 'static,
    T: Any + Send,
{
    spawn(
        ctx,
        Work::Blocking(Box::new(f)),
        WorkerOptions::default(),
        false,
    )
}
