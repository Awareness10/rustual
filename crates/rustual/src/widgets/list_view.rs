//! `ListView` and `ListItem` (port of `textual/widgets/_list_view.py` and `_list_item.py`).
//!
//! A `ListView` is a vertical scrolling container of [`ListItem`] children; each item wraps
//! arbitrary widgets:
//!
//! ```ignore
//! ListView::new().id("menu").with([
//!     ListItem::new().with(compose![Label::new("One")]),
//!     ListItem::label("Two"),
//! ])
//! ```
//!
//! The highlighted item gets the `-highlight` class. Operations that change the children
//! (`append`, `insert`, `clear`, `pop`, ...) need the list's [`Ctx`]:
//! `ctx.with::<ListView, _>("#menu", |lv, ctx| lv.append(ListItem::label("x"), ctx))`.

use crate::binding::Binding;
use crate::context::Ctx;
use crate::dom::NodeId;
use crate::events::Event;
use crate::message::{Flow, Message};
use crate::widget::{Child, Widget, WidgetExt};
use crate::widgets::Label;

const DEFAULT_CSS: &str = r#"
ListView {
    width: 1fr;
    height: 1fr;
    layout: vertical;
    overflow-x: hidden;
    overflow-y: auto;
}
ListView {
    background: $surface;
    & > ListItem {
        color: $foreground;
        height: auto;
        overflow: hidden hidden;
        width: 1fr;

        /* Textual toggles a `-hovered` class; `:hover` here also matches when a
           descendant of the item is under the mouse. */
        &:hover {
            background: $block-hover-background;
        }

        &.-highlight {
            color: $block-cursor-blurred-foreground;
            background: $block-cursor-blurred-background;
            text-style: $block-cursor-blurred-text-style;
        }
    }

    &:focus {
        background-tint: $foreground 5%;
        & > ListItem.-highlight {
            color: $block-cursor-foreground;
            background: $block-cursor-background;
            text-style: $block-cursor-text-style;
        }
    }

}
"#;

/// Posted when the highlighted item changes (Textual: `ListView.Highlighted`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Highlighted {
    /// Index of the highlighted item, `None` if nothing is highlighted.
    pub index: Option<usize>,
    /// The highlighted item's node.
    pub item: Option<NodeId>,
    /// The highlighted item's CSS id, if any.
    pub item_id: Option<String>,
}

/// Posted when an item is selected with enter or a click (Textual: `ListView.Selected`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selected {
    pub index: usize,
    pub item: NodeId,
    pub item_id: Option<String>,
}

/// Posted by a [`ListItem`] when clicked, handled by its `ListView`.
#[derive(Debug, Clone, Copy)]
struct ChildClicked;

/// An item in a [`ListView`]; give it children with `.with([...])`.
#[derive(Debug, Default, Clone)]
pub struct ListItem;

impl ListItem {
    pub fn new() -> Self {
        ListItem
    }

    /// Convenience: a `ListItem` containing a single [`Label`] with `markup`.
    pub fn label(markup: impl Into<String>) -> Child {
        ListItem.with([Label::new(markup).into()])
    }
}

impl Widget for ListItem {
    fn type_names(&self) -> &'static [&'static str] {
        &["ListItem"]
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            ctx.post_message(ChildClicked);
        }
        Flow::Continue
    }
}

/// A vertical list of [`ListItem`]s with a keyboard / mouse highlight.
#[derive(Debug, Clone)]
pub struct ListView {
    index: Option<usize>,
    initial_index: Option<usize>,
}

impl Default for ListView {
    fn default() -> Self {
        Self::new()
    }
}

impl ListView {
    pub fn new() -> Self {
        ListView {
            index: None,
            initial_index: Some(0),
        }
    }

    /// Index highlighted on mount (default `Some(0)`).
    pub fn initial_index(mut self, index: Option<usize>) -> Self {
        self.initial_index = index;
        self
    }

    /// The highlighted index.
    pub fn index(&self) -> Option<usize> {
        self.index
    }

    /// Item nodes in order.
    pub fn items(&self, ctx: &Ctx) -> Vec<NodeId> {
        ctx.children()
            .into_iter()
            .filter(|c| ctx.dom().widget::<ListItem>(*c).is_some())
            .collect()
    }

    /// Number of items.
    pub fn len(&self, ctx: &Ctx) -> usize {
        self.items(ctx).len()
    }

    pub fn is_empty(&self, ctx: &Ctx) -> bool {
        self.len(ctx) == 0
    }

    /// The highlighted item node.
    pub fn highlighted_child(&self, ctx: &Ctx) -> Option<NodeId> {
        self.index.and_then(|i| self.items(ctx).get(i).copied())
    }

    /// Set the highlighted index (clamped), updating classes, scrolling it into view and
    /// posting [`Highlighted`] if it changed.
    pub fn set_index(&mut self, index: Option<usize>, ctx: &mut Ctx) {
        let items = self.items(ctx);
        self.set_index_in(index, &items, false, ctx);
    }

    fn set_index_in(&mut self, index: Option<usize>, items: &[NodeId], force: bool, ctx: &mut Ctx) {
        let new = match index {
            _ if items.is_empty() => None,
            None => None,
            Some(i) => Some(i.min(items.len() - 1)),
        };
        let old = self.index;
        self.index = new;
        if old == new && !force {
            return;
        }
        // watch_index
        for (i, item) in items.iter().enumerate() {
            let on = Some(i) == new && !ctx.dom().is_disabled(*item);
            ctx.set_class_on(*item, on, "-highlight");
        }
        match new.map(|i| items[i]) {
            Some(item) => {
                ctx.with_id::<ListItem, _>(item, |_, ctx| ctx.scroll_visible());
                if ctx.dom().is_disabled(item) {
                    ctx.post_message(Highlighted {
                        index: None,
                        item: None,
                        item_id: None,
                    });
                } else {
                    let item_id = ctx.css_id(item).map(String::from);
                    ctx.post_message(Highlighted {
                        index: new,
                        item: Some(item),
                        item_id,
                    });
                }
            }
            None => ctx.post_message(Highlighted {
                index: None,
                item: None,
                item_id: None,
            }),
        }
    }

    /// Append an item at the end.
    pub fn append(&mut self, item: impl Into<Child>, ctx: &mut Ctx) -> NodeId {
        ctx.mount(item)
    }

    /// Append several items.
    pub fn extend(&mut self, items: impl IntoIterator<Item = Child>, ctx: &mut Ctx) -> Vec<NodeId> {
        items.into_iter().map(|i| ctx.mount(i)).collect()
    }

    /// Insert items before position `index`.
    pub fn insert(
        &mut self,
        index: usize,
        items: impl IntoIterator<Item = Child>,
        ctx: &mut Ctx,
    ) -> Vec<NodeId> {
        let me = ctx.id();
        let mut out = Vec::new();
        for (n, item) in items.into_iter().enumerate() {
            let at = self.child_position(index + n, ctx);
            out.push(ctx.mount_to(me, item, Some(at)));
        }
        // Keep the same item highlighted.
        if let Some(i) = self.index.filter(|i| *i >= index) {
            self.index = Some(i + out.len());
        }
        out
    }

    /// DOM child position of item `index` (items may be interleaved with other children).
    fn child_position(&self, index: usize, ctx: &Ctx) -> usize {
        let children = ctx.children();
        let mut seen = 0;
        for (pos, c) in children.iter().enumerate() {
            if ctx.dom().widget::<ListItem>(*c).is_some() {
                if seen == index {
                    return pos;
                }
                seen += 1;
            }
        }
        children.len()
    }

    /// Remove all items.
    pub fn clear(&mut self, ctx: &mut Ctx) {
        for item in self.items(ctx) {
            ctx.remove(item);
        }
        self.set_index_in(None, &[], false, ctx);
    }

    /// Remove the item at `index` (negative counts from the end; `None` = last).
    /// Returns the removed node, or `None` if the list is empty / the index is out of range.
    pub fn pop(&mut self, index: Option<isize>, ctx: &mut Ctx) -> Option<NodeId> {
        let items = self.items(ctx);
        let len = items.len() as isize;
        let i = index.unwrap_or(-1);
        let i = if i < 0 { i + len } else { i };
        if i < 0 || i >= len {
            return None;
        }
        let removed = self.remove_indices(&[i as usize], ctx);
        removed.into_iter().next()
    }

    /// Remove items at the given indices (negative counts from the end).
    pub fn remove_items(&mut self, indices: &[isize], ctx: &mut Ctx) -> Vec<NodeId> {
        let len = self.len(ctx) as isize;
        let normalized: Vec<usize> = indices
            .iter()
            .map(|i| if *i < 0 { i + len } else { *i })
            .filter(|i| *i >= 0 && *i < len)
            .map(|i| i as usize)
            .collect();
        self.remove_indices(&normalized, ctx)
    }

    fn remove_indices(&mut self, indices: &[usize], ctx: &mut Ctx) -> Vec<NodeId> {
        let items = self.items(ctx);
        let removed: Vec<NodeId> = indices
            .iter()
            .filter_map(|i| items.get(*i).copied())
            .collect();
        for r in &removed {
            ctx.remove(*r);
        }
        let remaining: Vec<NodeId> = items
            .iter()
            .copied()
            .filter(|i| !removed.contains(i))
            .collect();
        if let Some(index) = self.index {
            let before = indices.iter().filter(|i| **i < index).count();
            if before > 0 {
                // The highlighted item moved up; keep it highlighted.
                self.index = Some(index - before);
            } else if indices.contains(&index) {
                self.set_index_in(Some(index), &remaining, true, ctx);
            }
        }
        removed
    }

    fn enabled(ctx: &Ctx, items: &[NodeId], i: usize) -> bool {
        !ctx.dom().is_disabled(items[i])
    }

    /// Highlight the next enabled item (no wrapping).
    pub fn action_cursor_down(&mut self, ctx: &mut Ctx) {
        let items = self.items(ctx);
        match self.index {
            None if !items.is_empty() => self.set_index_in(Some(0), &items, false, ctx),
            None => {}
            Some(i) => {
                if let Some(n) = (i + 1..items.len()).find(|n| Self::enabled(ctx, &items, *n)) {
                    self.set_index_in(Some(n), &items, false, ctx);
                }
            }
        }
    }

    /// Highlight the previous enabled item (no wrapping).
    pub fn action_cursor_up(&mut self, ctx: &mut Ctx) {
        let items = self.items(ctx);
        match self.index {
            None if !items.is_empty() => {
                self.set_index_in(Some(items.len() - 1), &items, false, ctx)
            }
            None => {}
            Some(i) => {
                if let Some(n) = (0..i).rev().find(|n| Self::enabled(ctx, &items, *n)) {
                    self.set_index_in(Some(n), &items, false, ctx);
                }
            }
        }
    }

    fn first_enabled(&mut self, forward: bool, ctx: &mut Ctx) {
        let items = self.items(ctx);
        let found = if forward {
            (0..items.len()).find(|n| Self::enabled(ctx, &items, *n))
        } else {
            (0..items.len())
                .rev()
                .find(|n| Self::enabled(ctx, &items, *n))
        };
        if found.is_some() {
            self.set_index_in(found, &items, false, ctx);
        }
    }

    /// Move the highlight by roughly one page (the viewport height) in `direction` (-1 / 1).
    fn page(&mut self, direction: i32, ctx: &mut Ctx) {
        let items = self.items(ctx);
        if items.is_empty() {
            return;
        }
        let Some(cur) = self.index else {
            return self.first_enabled(direction > 0, ctx);
        };
        let height = ctx.size().height.max(1);
        let cur_y = ctx.region_of(items[cur]).y;
        let target_y = cur_y + direction * height;
        let mut target = cur;
        if direction > 0 {
            for (i, item) in items.iter().enumerate().skip(cur) {
                if ctx.region_of(*item).y > target_y {
                    break;
                }
                target = i;
            }
        } else {
            for (i, item) in items.iter().enumerate().take(cur + 1).rev() {
                if ctx.region_of(*item).y < target_y {
                    break;
                }
                target = i;
            }
        }
        // Avoid landing on a disabled item.
        let pick = if direction > 0 {
            (target..items.len())
                .find(|n| Self::enabled(ctx, &items, *n))
                .or_else(|| (0..target).rev().find(|n| Self::enabled(ctx, &items, *n)))
        } else {
            (0..=target)
                .rev()
                .find(|n| Self::enabled(ctx, &items, *n))
                .or_else(|| (target..items.len()).find(|n| Self::enabled(ctx, &items, *n)))
        };
        if pick.is_some() {
            self.set_index_in(pick, &items, false, ctx);
        }
    }

    /// Post [`Selected`] for the highlighted item.
    pub fn action_select_cursor(&mut self, ctx: &mut Ctx) {
        let Some(index) = self.index else { return };
        let Some(item) = self.items(ctx).get(index).copied() else {
            return;
        };
        let item_id = ctx.css_id(item).map(String::from);
        ctx.post_message(Selected {
            index,
            item,
            item_id,
        });
    }
}

impl Widget for ListView {
    fn type_names(&self) -> &'static [&'static str] {
        &["ListView", "VerticalScroll", "ScrollableContainer"]
    }

    fn allow_maximize(&self) -> Option<bool> {
        Some(true)
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn can_focus_children(&self) -> bool {
        false
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        let items = self.items(ctx);
        if let Some(mut index) = self.initial_index {
            if items.is_empty() {
                return;
            }
            if index >= items.len() {
                index = 0;
            }
            if ctx.dom().is_disabled(items[index]) {
                let n = items.len();
                if let Some(i) = (0..n)
                    .map(|k| (index + k) % n)
                    .find(|i| Self::enabled(ctx, &items, *i))
                {
                    index = i;
                }
            }
            self.set_index_in(Some(index), &items, true, ctx);
        }
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("enter", "select_cursor", "Select").hidden(),
            Binding::new("up", "cursor_up", "Cursor up").hidden(),
            Binding::new("down", "cursor_down", "Cursor down").hidden(),
            Binding::new("home", "first", "First").hidden(),
            Binding::new("end", "last", "Last").hidden(),
            Binding::new("pageup", "page_up", "Page up").hidden(),
            Binding::new("pagedown", "page_down", "Page down").hidden(),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "select_cursor" => self.action_select_cursor(ctx),
            "cursor_up" => self.action_cursor_up(ctx),
            "cursor_down" => self.action_cursor_down(ctx),
            "first" => self.first_enabled(true, ctx),
            "last" => self.first_enabled(false, ctx),
            "page_up" => self.page(-1, ctx),
            "page_down" => self.page(1, ctx),
            _ => return false,
        }
        true
    }

    fn on_message(&mut self, message: &Message, ctx: &mut Ctx) -> Flow {
        if message.is::<ChildClicked>() {
            let items = self.items(ctx);
            if let Some(index) = items.iter().position(|i| *i == message.sender) {
                ctx.focus_self();
                self.set_index_in(Some(index), &items, false, ctx);
                let item = items[index];
                let item_id = ctx.css_id(item).map(String::from);
                ctx.post_message(Selected {
                    index,
                    item,
                    item_id,
                });
            }
            return Flow::Stop;
        }
        Flow::Continue
    }
}
