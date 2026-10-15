//! Grid layout: port of `layouts/grid.py`.
//!
//! - `grid-size: <columns> [<rows>]` (rows 0 = as many as needed).
//! - `grid-columns` / `grid-rows` repeat cyclically; default `1fr` columns, rows `1fr`
//!   (or `auto` when the container has auto height / no height).
//! - `auto` columns/rows take the max content size of the single-span cells in them.
//! - `grid-gutter`: as in Textual, `grid_gutter_horizontal` separates *rows* and
//!   `grid_gutter_vertical` separates *columns*.
//! - Children fill cells row by row, skipping cells occupied by earlier spans.

use super::arrange::{is_absolute, resolve_offset, Placement};
use super::box_model::{floor, resolve_scalar, resolve_tracks, LayoutCx};
use crate::dom::NodeId;
use rustual_css::{Dimension, Scalar};
use rustual_render::{Region, Size};
use std::collections::HashMap;

fn repeat_scalars(scalars: &[Scalar], count: usize) -> Vec<Scalar> {
    if scalars.is_empty() {
        return vec![Scalar::fr(1.0); count];
    }
    scalars.iter().copied().cycle().take(count).collect()
}

pub(crate) fn arrange(
    cx: &LayoutCx,
    parent: NodeId,
    children: &[NodeId],
    size: Size,
    greedy: bool,
) -> Vec<Placement> {
    let styles = cx.styles(parent);
    let viewport = cx.viewport;
    let parent_auto_height = matches!(styles.height, Some(h) if h.is_auto());
    let row_scalars: Vec<Scalar> = match &styles.grid_rows {
        Some(r) if !r.is_empty() => r.clone(),
        _ if size.height != 0 && !parent_auto_height => vec![Scalar::fr(1.0)],
        _ => vec![Scalar::AUTO],
    };
    let column_scalars: Vec<Scalar> = match &styles.grid_columns {
        Some(c) if !c.is_empty() => c.clone(),
        _ => vec![Scalar::fr(1.0)],
    };
    let (gutter_horizontal, gutter_vertical) = styles.get_grid_gutter();
    let (size_columns, size_rows) = styles.get_grid_size();
    let table_columns = size_columns.max(1);

    // Place cells.
    let mut cell_map: HashMap<(i32, i32), NodeId> = HashMap::new();
    let mut cells: Vec<(NodeId, i32, i32, i32, i32)> = Vec::new();
    let mut coord_index: i32 = 0;
    let coord = |i: i32| (i % table_columns, i / table_columns);
    let mut row = 0;
    for &child in children {
        let st = cx.styles(child);
        let column_span = st.column_span.unwrap_or(1).max(1);
        let row_span = st.row_span.unwrap_or(1).max(1);
        loop {
            let (column, r) = coord(coord_index);
            row = r;
            let free = (column..column + column_span)
                .all(|c| (r..r + row_span).all(|rr| !cell_map.contains_key(&(c, rr))));
            if free {
                for c in column..column + column_span {
                    for rr in r..r + row_span {
                        cell_map.insert((c, rr), child);
                    }
                }
                cells.push((child, column, r, column_span - 1, row_span - 1));
                break;
            }
            coord_index += 1;
        }
        coord_index += 1;
    }

    let mut column_scalars = repeat_scalars(&column_scalars, table_columns as usize);
    let table_rows = if size_rows != 0 { size_rows } else { row + 1 };
    let mut row_scalars = repeat_scalars(&row_scalars, table_rows.max(0) as usize);

    let width_limits = |id: NodeId, width: i32| -> i32 {
        let st = cx.styles(id);
        let mut width = width;
        if let Some(m) = st.min_width {
            width = width
                .max(resolve_scalar(&m, Dimension::Width, size, viewport, width as f64) as i32);
        }
        if let Some(m) = st.max_width {
            width = width
                .min(resolve_scalar(&m, Dimension::Width, size, viewport, width as f64) as i32);
        }
        width
    };
    let height_limits = |id: NodeId, height: i32| -> i32 {
        let st = cx.styles(id);
        let mut height = height;
        if let Some(m) = st.min_height {
            height = height
                .max(resolve_scalar(&m, Dimension::Height, size, viewport, height as f64) as i32);
        }
        if let Some(m) = st.max_height {
            height = height
                .min(resolve_scalar(&m, Dimension::Height, size, viewport, height as f64) as i32);
        }
        height
    };

    // Auto columns.
    for (column, scalar) in column_scalars.iter_mut().enumerate() {
        if !scalar.is_auto() {
            continue;
        }
        let mut width = 0;
        for r in 0..row_scalars.len() as i32 {
            if let Some(&w) = cell_map.get(&(column as i32, r)) {
                let st = cx.styles(w);
                if st.column_span.unwrap_or(1) != 1 {
                    continue;
                }
                width = width.max(width_limits(
                    w,
                    cx.content_width(w, size) + st.gutter().width(),
                ));
            }
        }
        *scalar = Scalar::cells(width as f32);
    }

    let columns = resolve_tracks(
        &column_scalars,
        size.width,
        gutter_vertical,
        size,
        viewport,
        Dimension::Width,
    );

    // Auto rows.
    for (r, scalar) in row_scalars.iter_mut().enumerate() {
        if !scalar.is_auto() {
            continue;
        }
        let mut height = 0;
        for column in 0..column_scalars.len() as i32 {
            if let Some(&w) = cell_map.get(&(column, r as i32)) {
                let st = cx.styles(w);
                if st.row_span.unwrap_or(1) != 1 {
                    continue;
                }
                let column_width = columns[column as usize].1;
                let g = st.gutter();
                let h = cx.content_height(w, size, column_width - g.width()) + g.height();
                height = height.max(height_limits(w, h));
            }
        }
        *scalar = Scalar::cells(height as f32);
    }

    let rows = resolve_tracks(
        &row_scalars,
        size.height,
        gutter_horizontal,
        size,
        viewport,
        Dimension::Height,
    );

    let mut placements = Vec::with_capacity(cells.len());
    if columns.is_empty() || rows.is_empty() {
        return placements;
    }
    let max_column = columns.len() - 1;
    let max_row = rows.len() - 1;
    for (id, column, r, column_span, row_span) in cells {
        if r as usize > max_row {
            break;
        }
        let x = columns[column as usize].0;
        let y = rows[r as usize].0;
        let (x2, cell_width) = columns[max_column.min((column + column_span) as usize)];
        let (y2, cell_height) = rows[max_row.min((r + row_span) as usize)];
        let cell_size = Size::new(cell_width + x2 - x, cell_height + y2 - y);
        let bm = cx.box_model(
            id,
            cell_size,
            cell_size.width as f64,
            cell_size.height as f64,
            true,
            greedy,
        );
        let full = Region::new(
            x,
            y,
            floor(bm.width + bm.margin.width() as f64),
            floor(bm.height + bm.margin.height() as f64),
        );
        // `crop_size(cell_size).shrink(margin)`
        let region = Region::new(
            full.x,
            full.y,
            full.width.min(cell_size.width),
            full.height.min(cell_size.height),
        )
        .shrink(bm.margin);
        let st = cx.styles(id);
        placements.push(Placement {
            node: id,
            region,
            offset: resolve_offset(cx, st, cell_size),
            margin: bm.margin,
            z: 0,
            fixed: false,
            absolute: is_absolute(st),
        });
    }
    placements
}
