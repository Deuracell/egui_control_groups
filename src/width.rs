//! Whole-group width control shared by the widgets.

/// How wide a whole group should be.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Width {
    /// Natural size from the cells (the default).
    #[default]
    Auto,
    /// Exactly this many points.
    Fixed(f32),
    /// All of `ui.available_width()`.
    Fill,
}

impl Width {
    /// The requested total width, or `None` for natural sizing. A fixed width
    /// that is not finite, or a fill with no finite available width, falls
    /// back to `None`.
    pub(crate) fn resolve(self, available: f32) -> Option<f32> {
        let w = match self {
            Self::Auto => return None,
            Self::Fixed(w) => w,
            Self::Fill => available,
        };
        (w.is_finite() && w >= 0.0).then_some(w)
    }
}

/// Scale `naturals` so they sum to `target`, in proportion to their sizes.
///
/// No cell gets less than `min_each` (reduced to `target / n` when the group
/// is too narrow for that, so the total is always exactly `target`).
pub(crate) fn distribute(naturals: &[f32], target: f32, min_each: f32) -> Vec<f32> {
    let n = naturals.len();
    if n == 0 {
        return Vec::new();
    }
    let floor = min_each.min(target / n as f32).max(0.0);
    let mut out = vec![0.0_f32; n];
    let mut pinned = vec![false; n];
    // Cells that would fall below the floor are pinned to it and the rest are
    // rescaled; repeat until stable (at most `n` rounds).
    for _ in 0..=n {
        let free_target = target - floor * pinned.iter().filter(|p| **p).count() as f32;
        let free_sum: f32 = (0..n).filter(|&i| !pinned[i]).map(|i| naturals[i]).sum();
        let free_count = pinned.iter().filter(|p| !**p).count();
        let mut again = false;
        for i in 0..n {
            if pinned[i] {
                out[i] = floor;
                continue;
            }
            out[i] = if free_sum > 0.0 {
                free_target * naturals[i] / free_sum
            } else {
                free_target / free_count as f32
            };
            if out[i] < floor {
                pinned[i] = true;
                again = true;
            }
        }
        if !again {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_guards_non_finite() {
        assert_eq!(Width::Auto.resolve(100.0), None);
        assert_eq!(Width::Fixed(50.0).resolve(f32::INFINITY), Some(50.0));
        assert_eq!(Width::Fixed(f32::NAN).resolve(100.0), None);
        assert_eq!(Width::Fill.resolve(f32::INFINITY), None);
        assert_eq!(Width::Fill.resolve(f32::NAN), None);
        assert_eq!(Width::Fill.resolve(120.0), Some(120.0));
    }

    #[test]
    fn distribute_is_proportional_and_exact() {
        let v = distribute(&[10.0, 30.0], 80.0, 1.0);
        assert_eq!(v, vec![20.0, 60.0]);
        let v = distribute(&[40.0, 40.0, 40.0], 60.0, 1.0);
        assert!((v.iter().sum::<f32>() - 60.0).abs() < 1e-4);
    }

    #[test]
    fn distribute_respects_minimum() {
        let v = distribute(&[2.0, 100.0], 52.0, 10.0);
        assert!((v[0] - 10.0).abs() < 1e-4);
        assert!((v.iter().sum::<f32>() - 52.0).abs() < 1e-4);
        // Too narrow for the minimum: shared equally, total still exact.
        let v = distribute(&[5.0, 50.0], 8.0, 10.0);
        assert!((v[0] - 4.0).abs() < 1e-4 && (v[1] - 4.0).abs() < 1e-4);
    }
}
