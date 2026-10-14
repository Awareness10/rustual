//! Compositor: paints laid-out widgets onto a [`Canvas`].
//!
//! For each node in [`Arrangement::order`] (clipped to `layout.clip`):
//! 1. Fill the border box with the background (`background` composited over what is beneath;
//!    `background-tint` and `tint` applied).
//! 2. Draw borders (`BorderKind::chars/locations`), border title/subtitle (markup, aligned per
//!    `border-title-align`, styled with `border-title-*`).
//! 3. Render content (`Widget::render`) into `content_region`: `Content::Text` is wrapped to the
//!    content width with `text-align`, `text-wrap`, `text-overflow`, then positioned by
//!    `content-align` (horizontal/vertical); `Content::Lines` drawn as-is. Base style is the
//!    widget's color (incl. `auto`) and `text-style` (see `widget::style_from_styles`).
//!    `text-opacity` blends text toward the background.
//! 4. Draw scrollbars (vertical: right edge, horizontal: bottom edge, corner) using
//!    `scrollbar-*` colors (hover/active variants not required) with Textual's thumb glyphs.
//! 5. Draw outline over the border box edge.
//! 6. `opacity` < 1 blends the node's painted area toward the background beneath.
//!
//! Screens: `layers` lists screens bottom → top; each is painted fully in turn (a modal screen's
//! semi-transparent background lets the screen beneath show through).

use crate::dom::{Dom, NodeId};
use crate::layout::Arrangement;
use crate::widget::RenderEnv;
use textual_render::{Canvas, Size};

pub fn render(dom: &Dom, env: &RenderEnv, layers: &[(NodeId, &Arrangement)], size: Size) -> Canvas {
    let _ = (dom, env, layers, size);
    todo!("compositor")
}
