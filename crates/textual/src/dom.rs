//! The DOM: an arena of nodes, each owning a widget, CSS identity and layout results.

use crate::widget::Widget;
use slotmap::SlotMap;
use textual_css::{SelectorList, SelectorNode, Styles};
use textual_render::{Offset, Region, Size};

slotmap::new_key_type! {
    /// Handle to a node in the [`Dom`]. Stale handles (removed nodes) simply resolve to `None`.
    pub struct NodeId;
}

/// Results of layout for one node. All regions are absolute screen coordinates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeLayout {
    /// Border box (after margins, offsets and ancestor scrolling), unclipped.
    pub region: Region,
    /// Area inside border + padding, excluding scrollbars. Widget content renders here.
    pub content_region: Region,
    /// Visible part of `region` (intersection with ancestors' clip areas).
    pub clip: Region,
    /// Size of scrollable content (children extent, or the widget's own `virtual_size`).
    pub virtual_size: Size,
    /// Whether vertical / horizontal scrollbars are shown.
    pub scrollbar_v: bool,
    pub scrollbar_h: bool,
    /// Displayed and at least partially within its clip.
    pub visible: bool,
}

impl NodeLayout {
    /// Maximum scroll offset given virtual size and the content (viewport) region.
    pub fn max_scroll(&self) -> Offset {
        Offset::new(
            (self.virtual_size.width - self.content_region.width).max(0),
            (self.virtual_size.height - self.content_region.height).max(0),
        )
    }
}

pub struct Node {
    pub(crate) widget: Option<Box<dyn Widget>>,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub css_id: Option<String>,
    pub classes: Vec<String>,
    /// CSS type names, most specific first, always ending in `"Widget"`.
    pub type_names: Vec<&'static str>,
    /// Styles set programmatically (highest precedence after `!important`).
    pub inline_styles: Styles,
    /// Computed styles (cascade + inline + inheritance), refreshed every frame.
    pub styles: Styles,
    pub disabled: bool,
    pub border_title: Option<String>,
    pub border_subtitle: Option<String>,
    pub layout: NodeLayout,
    /// Current scroll position for scrollable nodes.
    pub scroll_offset: Offset,
    pub mounted: bool,
    /// Direct child of the app root.
    pub is_screen: bool,
    /// A modal screen: the screen beneath stays visible.
    pub is_modal: bool,
    /// Cached `Widget::can_focus` (refreshed whenever the widget is checked back in).
    pub can_focus: bool,
    /// Cached `Widget::can_focus_children`.
    pub can_focus_children: bool,
    /// Tooltip text (markup) shown when the mouse rests over the widget (see `crate::tooltip`).
    pub tooltip: Option<String>,
    /// In a loading state ([`Ctx::set_loading`](crate::Ctx::set_loading)): covered by
    /// [`cover`](Self::cover), excluded from the focus chain, input blocked.
    pub loading: bool,
    /// Widget painted over this one in its place (the loading indicator). Its `parent` is this
    /// node, but it is not in `children`; [`Dom::descendants`] includes it after the children.
    pub cover: Option<NodeId>,
    /// This node is another node's [`cover`](Self::cover).
    pub is_cover: bool,
    /// Screens only: the maximized widget ([`Ctx::maximize`](crate::Ctx::maximize)).
    pub maximized: Option<NodeId>,
}

impl Node {
    pub fn new(widget: Box<dyn Widget>) -> Node {
        let mut type_names: Vec<&'static str> = widget.type_names().to_vec();
        if !type_names.contains(&"Widget") {
            type_names.push("Widget");
        }
        let can_focus = widget.can_focus();
        let can_focus_children = widget.can_focus_children();
        Node {
            widget: Some(widget),
            parent: None,
            children: Vec::new(),
            css_id: None,
            classes: Vec::new(),
            type_names,
            inline_styles: Styles::default(),
            styles: Styles::default(),
            disabled: false,
            border_title: None,
            border_subtitle: None,
            layout: NodeLayout::default(),
            scroll_offset: Offset::ZERO,
            mounted: false,
            is_screen: false,
            is_modal: false,
            can_focus,
            can_focus_children,
            tooltip: None,
            loading: false,
            cover: None,
            is_cover: false,
            maximized: None,
        }
    }

    /// The widget, unless it is currently checked out by a running handler.
    pub fn widget(&self) -> Option<&dyn Widget> {
        self.widget.as_deref()
    }

    pub fn widget_mut(&mut self) -> Option<&mut (dyn Widget + 'static)> {
        self.widget.as_deref_mut()
    }

    pub fn has_class(&self, c: &str) -> bool {
        self.classes.iter().any(|x| x == c)
    }

    pub fn add_class(&mut self, c: &str) {
        for c in c.split_whitespace() {
            if !self.has_class(c) {
                self.classes.push(c.to_string());
            }
        }
    }

    pub fn remove_class(&mut self, c: &str) {
        for c in c.split_whitespace() {
            self.classes.retain(|x| x != c);
        }
    }

    pub fn set_class(&mut self, on: bool, c: &str) {
        if on {
            self.add_class(c)
        } else {
            self.remove_class(c)
        }
    }

    pub fn toggle_class(&mut self, c: &str) {
        for c in c.split_whitespace() {
            let on = !self.has_class(c);
            self.set_class(on, c);
        }
    }

    pub fn is_type(&self, name: &str) -> bool {
        self.type_names.contains(&name)
    }
}

/// Arena of nodes plus the interaction state needed for pseudo-class matching.
#[derive(Default)]
pub struct Dom {
    nodes: SlotMap<NodeId, Node>,
    pub root: Option<NodeId>,
    pub focused: Option<NodeId>,
    pub hovered: Option<NodeId>,
    pub dark: bool,
}

impl Dom {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, node: Node) -> NodeId {
        self.nodes.insert(node)
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Attach `child` as the last child of `parent` (detaching it from any previous parent).
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.insert_child(parent, child, usize::MAX);
    }

    /// Attach `child` at `index` among `parent`'s children (clamped).
    pub fn insert_child(&mut self, parent: NodeId, child: NodeId, index: usize) {
        self.detach(child);
        if let Some(p) = self.nodes.get_mut(parent) {
            let index = index.min(p.children.len());
            p.children.insert(index, child);
        }
        if let Some(c) = self.nodes.get_mut(child) {
            c.parent = Some(parent);
        }
    }

    /// Remove `child` from its parent's child list (the node stays in the arena).
    pub fn detach(&mut self, child: NodeId) {
        let parent = self.nodes.get(child).and_then(|c| c.parent);
        if let Some(p) = parent.and_then(|p| self.nodes.get_mut(p)) {
            p.children.retain(|c| *c != child);
            if p.cover == Some(child) {
                p.cover = None;
            }
        }
        if let Some(c) = self.nodes.get_mut(child) {
            c.parent = None;
        }
    }

    /// Remove a node and all its descendants. Returns removed ids in post-order (children first).
    pub fn remove_subtree(&mut self, id: NodeId) -> Vec<NodeId> {
        let ids: Vec<NodeId> = self.descendants_post_order(id);
        self.detach(id);
        for n in &ids {
            self.nodes.remove(*n);
            if self.focused == Some(*n) {
                self.focused = None;
            }
            if self.hovered == Some(*n) {
                self.hovered = None;
            }
        }
        ids
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(id).and_then(|n| n.parent)
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.nodes
            .get(id)
            .map(|n| n.children.as_slice())
            .unwrap_or(&[])
    }

    /// Ancestors from parent up to the root (excluding `id`).
    pub fn ancestors(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut cur = self.parent(id);
        while let Some(p) = cur {
            out.push(p);
            cur = self.parent(p);
        }
        out
    }

    /// `id` followed by its ancestors.
    pub fn ancestors_with_self(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = vec![id];
        out.extend(self.ancestors(id));
        out
    }

    pub fn is_ancestor(&self, ancestor: NodeId, id: NodeId) -> bool {
        let mut cur = self.parent(id);
        while let Some(p) = cur {
            if p == ancestor {
                return true;
            }
            cur = self.parent(p);
        }
        false
    }

    /// Pre-order DFS including `id` (a node's [`cover`](Node::cover) follows its children).
    pub fn descendants(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            if let Some(node) = self.nodes.get(n) {
                out.push(n);
                stack.extend(node.cover);
                stack.extend(node.children.iter().rev());
            }
        }
        out
    }

    fn descendants_post_order(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = self.descendants(id);
        out.reverse();
        out
    }

    /// The screen (child of root) containing `id`, if any.
    pub fn screen_of(&self, id: NodeId) -> Option<NodeId> {
        self.ancestors_with_self(id)
            .into_iter()
            .find(|n| self.nodes.get(*n).map(|n| n.is_screen).unwrap_or(false))
    }

    pub fn node_ref(&self, id: NodeId) -> NodeRef<'_> {
        NodeRef { dom: self, id }
    }

    /// All nodes under (and including) `root` matching `selector`, in document order.
    pub fn query(&self, root: NodeId, selector: &SelectorList) -> Vec<NodeId> {
        self.descendants(root)
            .into_iter()
            .filter(|n| selector.matches(self.node_ref(*n)))
            .collect()
    }

    /// Parse and run a selector query. Invalid selectors return an empty vec.
    pub fn query_str(&self, root: NodeId, selector: &str) -> Vec<NodeId> {
        match SelectorList::parse(selector) {
            Ok(sel) => self.query(root, &sel),
            Err(_) => Vec::new(),
        }
    }

    pub fn find_by_css_id(&self, root: NodeId, css_id: &str) -> Option<NodeId> {
        self.descendants(root)
            .into_iter()
            .find(|n| self.nodes.get(*n).and_then(|n| n.css_id.as_deref()) == Some(css_id))
    }

    pub fn widget<W: Widget>(&self, id: NodeId) -> Option<&W> {
        let w: &dyn Widget = self.nodes.get(id)?.widget.as_deref()?;
        crate::widget::downcast_ref::<W>(w)
    }

    pub fn widget_mut<W: Widget>(&mut self, id: NodeId) -> Option<&mut W> {
        let w: &mut dyn Widget = self.nodes.get_mut(id)?.widget.as_deref_mut()?;
        crate::widget::downcast_mut::<W>(w)
    }

    /// Whether the node or any ancestor is disabled.
    pub fn is_disabled(&self, id: NodeId) -> bool {
        self.ancestors_with_self(id)
            .iter()
            .any(|n| self.nodes.get(*n).map(|n| n.disabled).unwrap_or(false))
    }

    /// Whether input to `id` is blocked by a loading ancestor (or itself). Nodes inside a
    /// cover (the loading indicator) are not blocked.
    pub fn is_loading(&self, id: NodeId) -> bool {
        let mut cur = Some(id);
        while let Some(n) = cur.and_then(|c| self.nodes.get(c)) {
            if n.is_cover {
                return false;
            }
            if n.loading {
                return true;
            }
            cur = n.parent;
        }
        false
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &Node)> {
        self.nodes.iter()
    }
}

/// A `(dom, id)` handle implementing [`SelectorNode`] for CSS matching.
#[derive(Clone, Copy)]
pub struct NodeRef<'a> {
    pub dom: &'a Dom,
    pub id: NodeId,
}

impl<'a> NodeRef<'a> {
    fn node(&self) -> Option<&'a Node> {
        self.dom.nodes.get(self.id)
    }

    fn sibling_index(&self) -> Option<(usize, usize)> {
        let parent = self.dom.parent(self.id)?;
        let siblings: Vec<NodeId> = self
            .dom
            .children(parent)
            .iter()
            .copied()
            .filter(|c| {
                self.dom
                    .get(*c)
                    .map(|n| n.styles.get_display() != textual_css::Display::None)
                    .unwrap_or(false)
                    || *c == self.id
            })
            .collect();
        let idx = siblings.iter().position(|c| *c == self.id)?;
        Some((idx, siblings.len()))
    }
}

impl<'a> SelectorNode for NodeRef<'a> {
    fn type_names(&self) -> &[&'static str] {
        self.node().map(|n| n.type_names.as_slice()).unwrap_or(&[])
    }

    fn css_id(&self) -> Option<&str> {
        self.node().and_then(|n| n.css_id.as_deref())
    }

    fn has_class(&self, class: &str) -> bool {
        self.node().map(|n| n.has_class(class)).unwrap_or(false)
    }

    fn has_pseudo_class(&self, pseudo: &str) -> bool {
        let dom = self.dom;
        match pseudo {
            "focus" => dom.focused == Some(self.id),
            "blur" => dom.focused != Some(self.id),
            "focus-within" => dom
                .focused
                .map(|f| f == self.id || dom.is_ancestor(self.id, f))
                .unwrap_or(false),
            "hover" => dom
                .hovered
                .map(|h| h == self.id || dom.is_ancestor(self.id, h))
                .unwrap_or(false),
            "disabled" => dom.is_disabled(self.id),
            "enabled" => !dom.is_disabled(self.id),
            "dark" => dom.dark,
            "light" => !dom.dark,
            "can-focus" => self.node().map(|n| n.can_focus).unwrap_or(false),
            "empty" => dom.children(self.id).is_empty(),
            "first-child" | "first-of-type" => {
                self.sibling_index().map(|(i, _)| i == 0).unwrap_or(false)
            }
            "last-child" | "last-of-type" => self
                .sibling_index()
                .map(|(i, n)| i + 1 == n)
                .unwrap_or(false),
            // Textual counts from 1: the first child is odd.
            "odd" => self
                .sibling_index()
                .map(|(i, _)| i % 2 == 0)
                .unwrap_or(false),
            "even" => self
                .sibling_index()
                .map(|(i, _)| i % 2 == 1)
                .unwrap_or(false),
            _ => false,
        }
    }

    fn parent(&self) -> Option<Self> {
        self.dom.parent(self.id).map(|p| NodeRef {
            dom: self.dom,
            id: p,
        })
    }
}
