//! Integer space division (port of `rich/_ratio.py`). Rich uses Python's `round` (half to
//! even) and float division; both are reproduced so widths match `rich` exactly.

/// Reduce `values` by a total of `total`, proportionally to `ratios`, each by at most its
/// `maximums` entry (`ratio_reduce`).
pub(crate) fn ratio_reduce(
    total: i32,
    ratios: &[i32],
    maximums: &[i32],
    values: &[i32],
) -> Vec<i32> {
    let ratios: Vec<i32> = ratios
        .iter()
        .zip(maximums)
        .map(|(r, m)| if *m != 0 { *r } else { 0 })
        .collect();
    let mut total_ratio: i32 = ratios.iter().sum();
    if total_ratio == 0 {
        return values.to_vec();
    }
    let mut total_remaining = total;
    let mut out = Vec::with_capacity(values.len());
    for ((ratio, maximum), value) in ratios.iter().zip(maximums).zip(values) {
        if *ratio != 0 && total_ratio > 0 {
            let share =
                (*ratio as f64 * total_remaining as f64 / total_ratio as f64).round_ties_even();
            let distributed = (*maximum).min(share as i32);
            out.push(value - distributed);
            total_remaining -= distributed;
            total_ratio -= ratio;
        } else {
            out.push(*value);
        }
    }
    out
}

/// Divide `total` into parts proportional to `ratios`, each at least its `minimums` entry
/// (`ratio_distribute`).
pub(crate) fn ratio_distribute(total: i32, ratios: &[i32], minimums: Option<&[i32]>) -> Vec<i32> {
    let ratios: Vec<i32> = match minimums {
        Some(mins) => ratios
            .iter()
            .zip(mins)
            .map(|(r, m)| if *m != 0 { *r } else { 0 })
            .collect(),
        None => ratios.to_vec(),
    };
    let mut total_ratio: i32 = ratios.iter().sum();
    debug_assert!(total_ratio > 0, "sum of ratios must be > 0");
    let mut total_remaining = total;
    let mut out = Vec::with_capacity(ratios.len());
    for (i, ratio) in ratios.iter().enumerate() {
        let minimum = minimums.map_or(0, |m| m[i]);
        let distributed = if total_ratio > 0 {
            minimum.max((*ratio as f64 * total_remaining as f64 / total_ratio as f64).ceil() as i32)
        } else {
            total_remaining
        };
        out.push(distributed);
        total_ratio -= ratio;
        total_remaining -= distributed;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_rich() {
        // Values computed with rich 15 (`rich._ratio`).
        assert_eq!(ratio_distribute(10, &[1, 1, 1], None), vec![4, 3, 3]);
        assert_eq!(ratio_distribute(11, &[1, 2], Some(&[5, 1])), vec![5, 6]);
        assert_eq!(
            ratio_reduce(5, &[1, 1, 0], &[10, 10, 10], &[20, 20, 20]),
            vec![18, 17, 20]
        );
        assert_eq!(ratio_reduce(3, &[1, 1], &[1, 5], &[10, 10]), vec![9, 8]);
        // Half to even: 2.5 → 2.
        assert_eq!(ratio_reduce(5, &[1, 1], &[9, 9], &[9, 9]), vec![7, 6]);
    }
}
