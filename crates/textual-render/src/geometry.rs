//! Geometry primitives: offsets, sizes, regions and spacing.
//!
//! All coordinates are `i32` so intermediate layout arithmetic can go negative
//! (e.g. scrolled-off content). Sizes are expected to be non-negative.

use std::ops::{Add, Neg, Sub};

/// A point / displacement in cell coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Offset {
    pub x: i32,
    pub y: i32,
}

impl Offset {
    pub const ZERO: Offset = Offset { x: 0, y: 0 };

    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn is_origin(&self) -> bool {
        self.x == 0 && self.y == 0
    }

    /// Clamp each component into `0..=max` (component-wise, inclusive).
    pub fn clamp(&self, max_x: i32, max_y: i32) -> Offset {
        Offset::new(self.x.clamp(0, max_x.max(0)), self.y.clamp(0, max_y.max(0)))
    }
}

impl Add for Offset {
    type Output = Offset;
    fn add(self, o: Offset) -> Offset {
        Offset::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Offset {
    type Output = Offset;
    fn sub(self, o: Offset) -> Offset {
        Offset::new(self.x - o.x, self.y - o.y)
    }
}

impl Neg for Offset {
    type Output = Offset;
    fn neg(self) -> Offset {
        Offset::new(-self.x, -self.y)
    }
}

/// A width and height in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl Size {
    pub const ZERO: Size = Size { width: 0, height: 0 };

    pub const fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }

    pub fn area(&self) -> i32 {
        self.width.max(0) * self.height.max(0)
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    pub fn region(&self) -> Region {
        Region::new(0, 0, self.width, self.height)
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    /// Shrink by spacing (never below zero).
    pub fn shrink(&self, s: Spacing) -> Size {
        Size::new(
            (self.width - s.width()).max(0),
            (self.height - s.height()).max(0),
        )
    }

    pub fn grow(&self, s: Spacing) -> Size {
        Size::new(self.width + s.width(), self.height + s.height())
    }

    pub fn max(&self, o: Size) -> Size {
        Size::new(self.width.max(o.width), self.height.max(o.height))
    }

    pub fn min(&self, o: Size) -> Size {
        Size::new(self.width.min(o.width), self.height.min(o.height))
    }
}

/// A rectangular area. `x`/`y` is the top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Region {
    pub const ZERO: Region = Region { x: 0, y: 0, width: 0, height: 0 };

    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }

    pub fn from_offset_size(o: Offset, s: Size) -> Self {
        Self::new(o.x, o.y, s.width, s.height)
    }

    /// Build a region from two corners (exclusive bottom-right).
    pub fn from_corners(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self::new(x1, y1, (x2 - x1).max(0), (y2 - y1).max(0))
    }

    pub fn offset(&self) -> Offset {
        Offset::new(self.x, self.y)
    }

    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// Exclusive right edge.
    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    /// Exclusive bottom edge.
    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }

    pub fn area(&self) -> i32 {
        self.size().area()
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }

    pub fn contains_point(&self, p: Offset) -> bool {
        self.contains(p.x, p.y)
    }

    /// True if `other` lies entirely within self.
    pub fn contains_region(&self, other: &Region) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    pub fn overlaps(&self, other: &Region) -> bool {
        !self.intersection(other).is_empty()
    }

    /// Overlapping area (zero-sized region if none).
    pub fn intersection(&self, other: &Region) -> Region {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = self.right().min(other.right());
        let y2 = self.bottom().min(other.bottom());
        if x2 <= x1 || y2 <= y1 {
            Region::new(x1, y1, 0, 0)
        } else {
            Region::from_corners(x1, y1, x2, y2)
        }
    }

    /// Smallest region containing both.
    pub fn union(&self, other: &Region) -> Region {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        Region::from_corners(
            self.x.min(other.x),
            self.y.min(other.y),
            self.right().max(other.right()),
            self.bottom().max(other.bottom()),
        )
    }

    pub fn translate(&self, o: Offset) -> Region {
        Region::new(self.x + o.x, self.y + o.y, self.width, self.height)
    }

    /// Remove spacing from the edges (e.g. padding). Never below zero size.
    pub fn shrink(&self, s: Spacing) -> Region {
        Region::new(
            self.x + s.left,
            self.y + s.top,
            (self.width - s.width()).max(0),
            (self.height - s.height()).max(0),
        )
    }

    /// Add spacing around the edges (e.g. margin).
    pub fn grow(&self, s: Spacing) -> Region {
        Region::new(
            self.x - s.left,
            self.y - s.top,
            self.width + s.width(),
            self.height + s.height(),
        )
    }

    /// Clip to `0..width`, `0..height`.
    pub fn clip(&self, width: i32, height: i32) -> Region {
        self.intersection(&Region::new(0, 0, width, height))
    }

    /// Split vertically at `cut` cells from the left: (left, right).
    pub fn split_vertical(&self, cut: i32) -> (Region, Region) {
        let cut = cut.clamp(0, self.width);
        (
            Region::new(self.x, self.y, cut, self.height),
            Region::new(self.x + cut, self.y, self.width - cut, self.height),
        )
    }

    /// Split horizontally at `cut` cells from the top: (top, bottom).
    pub fn split_horizontal(&self, cut: i32) -> (Region, Region) {
        let cut = cut.clamp(0, self.height);
        (
            Region::new(self.x, self.y, self.width, cut),
            Region::new(self.x, self.y + cut, self.width, self.height - cut),
        )
    }

    /// Offset needed to scroll `self` (a viewport) so that `target` is visible.
    /// Both are in the same coordinate space. Returns the delta to apply to the scroll offset.
    pub fn get_scroll_to_visible(&self, target: &Region) -> Offset {
        let mut dx = 0;
        let mut dy = 0;
        if target.right() > self.right() {
            dx = target.right() - self.right();
        }
        if target.x < self.x + dx {
            dx = target.x - self.x;
        }
        if target.bottom() > self.bottom() {
            dy = target.bottom() - self.bottom();
        }
        if target.y < self.y + dy {
            dy = target.y - self.y;
        }
        Offset::new(dx, dy)
    }
}

/// Space around the edges of a box (margin, padding, border widths).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Spacing {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Spacing {
    pub const ZERO: Spacing = Spacing { top: 0, right: 0, bottom: 0, left: 0 };

    pub const fn new(top: i32, right: i32, bottom: i32, left: i32) -> Self {
        Self { top, right, bottom, left }
    }

    pub const fn all(n: i32) -> Self {
        Self::new(n, n, n, n)
    }

    /// `vertical` applies to top/bottom, `horizontal` to left/right.
    pub const fn symmetric(vertical: i32, horizontal: i32) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }

    /// CSS-style unpacking of 1, 2 or 4 values.
    pub fn unpack(values: &[i32]) -> Option<Self> {
        match values {
            [a] => Some(Self::all(*a)),
            [v, h] => Some(Self::symmetric(*v, *h)),
            [t, r, b, l] => Some(Self::new(*t, *r, *b, *l)),
            _ => None,
        }
    }

    /// Total horizontal spacing (left + right).
    pub fn width(&self) -> i32 {
        self.left + self.right
    }

    /// Total vertical spacing (top + bottom).
    pub fn height(&self) -> i32 {
        self.top + self.bottom
    }

    pub fn top_left(&self) -> Offset {
        Offset::new(self.left, self.top)
    }

    pub fn is_zero(&self) -> bool {
        *self == Spacing::ZERO
    }

    /// Component-wise max (used for margin collapsing).
    pub fn max(&self, o: Spacing) -> Spacing {
        Spacing::new(
            self.top.max(o.top),
            self.right.max(o.right),
            self.bottom.max(o.bottom),
            self.left.max(o.left),
        )
    }
}

impl Add for Spacing {
    type Output = Spacing;
    fn add(self, o: Spacing) -> Spacing {
        Spacing::new(self.top + o.top, self.right + o.right, self.bottom + o.bottom, self.left + o.left)
    }
}

impl Sub for Spacing {
    type Output = Spacing;
    fn sub(self, o: Spacing) -> Spacing {
        Spacing::new(self.top - o.top, self.right - o.right, self.bottom - o.bottom, self.left - o.left)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_and_union() {
        let a = Region::new(0, 0, 10, 10);
        let b = Region::new(5, 5, 10, 10);
        assert_eq!(a.intersection(&b), Region::new(5, 5, 5, 5));
        assert_eq!(a.union(&b), Region::new(0, 0, 15, 15));
        assert!(a.intersection(&Region::new(20, 20, 1, 1)).is_empty());
    }

    #[test]
    fn shrink_grow() {
        let r = Region::new(0, 0, 10, 10);
        let s = Spacing::new(1, 2, 3, 4);
        assert_eq!(r.shrink(s), Region::new(4, 1, 4, 6));
        assert_eq!(r.shrink(s).grow(s), r);
    }

    #[test]
    fn scroll_to_visible() {
        let vp = Region::new(0, 10, 20, 5);
        assert_eq!(vp.get_scroll_to_visible(&Region::new(0, 16, 1, 1)), Offset::new(0, 2));
        assert_eq!(vp.get_scroll_to_visible(&Region::new(0, 8, 1, 1)), Offset::new(0, -2));
        assert_eq!(vp.get_scroll_to_visible(&Region::new(0, 11, 1, 1)), Offset::ZERO);
    }

    #[test]
    fn spacing_unpack() {
        assert_eq!(Spacing::unpack(&[1, 2]), Some(Spacing::new(1, 2, 1, 2)));
        assert_eq!(Spacing::unpack(&[1, 2, 3]), None);
    }
}
