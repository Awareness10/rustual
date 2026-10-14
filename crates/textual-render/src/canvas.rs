//! A 2D grid of terminal cells with compositing and diffing.

use crate::color::Color;
use crate::geometry::{Region, Size};
use crate::strip::Strip;
use crate::style::{Attrs, Style};
use std::sync::Arc;

/// One terminal cell. Colors are always opaque once on a canvas.
/// A wide (2-cell) grapheme occupies its cell and the next cell has `symbol == ""`.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub symbol: String,
    pub fg: Color,
    pub bg: Color,
    pub attrs: Attrs,
    pub link: Option<Arc<str>>,
}

impl Cell {
    pub fn blank(bg: Color) -> Cell {
        Cell {
            symbol: " ".into(),
            fg: Color::WHITE,
            bg,
            attrs: Attrs::NONE,
            link: None,
        }
    }
}

/// A horizontal run of changed cells, starting at (x, y).
#[derive(Debug, Clone, PartialEq)]
pub struct CellRun {
    pub x: i32,
    pub y: i32,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Canvas {
    size: Size,
    cells: Vec<Cell>,
}

impl Canvas {
    pub fn new(size: Size, bg: Color) -> Canvas {
        todo!("{size:?}{bg}")
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn get(&self, x: i32, y: i32) -> Option<&Cell> {
        todo!("{x}{y}")
    }

    pub fn get_mut(&mut self, x: i32, y: i32) -> Option<&mut Cell> {
        todo!("{x}{y}")
    }

    /// Fill `region ∩ clip ∩ canvas` with spaces; the new bg is `existing_bg.composite(bg)`.
    /// fg is reset to `existing_bg.composite(bg)`'s contrast-neutral default (WHITE), attrs cleared.
    pub fn fill(&mut self, region: Region, clip: Region, bg: Color) {
        todo!("{region:?}{clip:?}{bg}")
    }

    /// Write a strip at (x, y), clipped to `clip`. Each span's style is `base.combine(span.style)`;
    /// a missing fg means WHITE, a missing bg keeps the cell's existing bg. Semi-transparent
    /// colors composite over the existing cell bg. Reverse attr is kept as an attr (not swapped).
    /// A wide grapheme that would straddle the clip edge is replaced by a space.
    pub fn put_strip(&mut self, x: i32, y: i32, strip: &Strip, clip: Region, base: &Style) {
        todo!("{x}{y}{strip:?}{clip:?}{base:?}")
    }

    pub fn put_str(&mut self, x: i32, y: i32, s: &str, style: &Style, clip: Region) {
        self.put_strip(x, y, &Strip::from_str(s, Style::default()), clip, style)
    }

    /// Blend fg and bg of every cell in region towards `color` by `factor` (used for opacity).
    pub fn blend_region(&mut self, region: Region, color: Color, factor: f32) {
        todo!("{region:?}{color}{factor}")
    }

    /// Tint fg and bg of every cell in region (see [`Color::tint`]).
    pub fn tint_region(&mut self, region: Region, tint: Color) {
        todo!("{region:?}{tint}")
    }

    /// Runs of cells that differ from `prev`. If sizes differ, every row is one full run.
    pub fn diff(&self, prev: &Canvas) -> Vec<CellRun> {
        todo!("{prev:?}")
    }

    /// Every row as a full run (for the first frame).
    pub fn full(&self) -> Vec<CellRun> {
        todo!()
    }

    /// Plain-text rows (wide-char continuation cells omitted), no trailing-space trimming.
    pub fn to_lines(&self) -> Vec<String> {
        todo!()
    }

    /// Rows with trailing whitespace trimmed, joined with `\n`. Handy for snapshot tests.
    pub fn to_text(&self) -> String {
        self.to_lines()
            .iter()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
