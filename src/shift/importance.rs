//! Incremental importance-weight validation and summary diagnostics.
//!
//! AGENTS.md Milestone 6 requires: non-negative finite weight validation
//! (delegated to [`NonNegative`]); diagnostics for the sum of weights,
//! sum of squared weights, effective sample size, minimum, and maximum;
//! and explicit failure for an all-zero or otherwise numerically
//! degenerate weight sequence, rather than a silent `NaN` or `0/0`.

use crate::error::RiskSieveError;
use crate::probability::NonNegative;

/// Accumulates non-negative importance weights one at a time, tracking
/// the summary statistics [`crate::anytime::shifted`] and other shifted
/// controllers need.
///
/// Sums are Kahan-compensated (AGENTS.md section 8: "use compensated or
/// pairwise summation for accumulated losses and weights").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightAccumulator {
    count: usize,
    sum: f64,
    sum_compensation: f64,
    sum_of_squares: f64,
    sum_of_squares_compensation: f64,
    min: f64,
    max: f64,
}

impl Default for WeightAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightAccumulator {
    /// Starts an empty accumulator.
    pub fn new() -> Self {
        Self {
            count: 0,
            sum: 0.0,
            sum_compensation: 0.0,
            sum_of_squares: 0.0,
            sum_of_squares_compensation: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        }
    }

    /// Folds in one more weight. Already validated non-negative and
    /// finite by construction (`weight: NonNegative`), so this cannot
    /// fail; reject a raw, caller-supplied value with
    /// [`NonNegative::new`] before calling this.
    pub fn update(&mut self, weight: NonNegative) {
        let w = weight.get();

        let adjusted = w - self.sum_compensation;
        let new_sum = self.sum + adjusted;
        self.sum_compensation = (new_sum - self.sum) - adjusted;
        self.sum = new_sum;

        let squared = w * w;
        let adjusted_sq = squared - self.sum_of_squares_compensation;
        let new_sum_sq = self.sum_of_squares + adjusted_sq;
        self.sum_of_squares_compensation = (new_sum_sq - self.sum_of_squares) - adjusted_sq;
        self.sum_of_squares = new_sum_sq;

        self.min = self.min.min(w);
        self.max = self.max.max(w);
        self.count += 1;
    }

    /// The number of weights folded in so far.
    pub fn count(&self) -> usize {
        self.count
    }

    /// The sum of weights folded in so far.
    pub fn sum(&self) -> f64 {
        self.sum
    }

    /// The sum of squared weights folded in so far (`W_n` in Hultberg,
    /// Zachariah, and Ribeiro 2026, Theorem 4.7).
    pub fn sum_of_squares(&self) -> f64 {
        self.sum_of_squares
    }

    /// The mean weight, or `0.0` before any weight arrives.
    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    /// `(min, max)` observed so far, or `None` before any weight arrives.
    pub fn range(&self) -> Option<(f64, f64)> {
        if self.count == 0 {
            None
        } else {
            Some((self.min, self.max))
        }
    }

    /// Kish's effective sample size: `(sum w_i)^2 / sum(w_i^2)`. `0.0`
    /// when the sum of squares is `0.0` (no weights yet, or all zero),
    /// rather than the `NaN` a direct `0.0 / 0.0` would produce.
    pub fn effective_sample_size(&self) -> f64 {
        if self.sum_of_squares == 0.0 {
            0.0
        } else {
            self.sum * self.sum / self.sum_of_squares
        }
    }

    /// Rejects an empty or all-zero weight sequence (AGENTS.md's
    /// "explicit failure for all-zero or numerically degenerate
    /// weights"). Checked against the *current* accumulated state, not
    /// latched permanently: a zero-weight observation followed by a
    /// positive-weight one is not degenerate once the positive weight
    /// arrives.
    pub fn ensure_not_degenerate(&self) -> Result<(), RiskSieveError> {
        if self.count == 0 || self.sum == 0.0 {
            return Err(RiskSieveError::DegenerateWeights);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weight(v: f64) -> NonNegative {
        NonNegative::new("weight", v).unwrap()
    }

    #[test]
    fn empty_accumulator_is_degenerate() {
        let acc = WeightAccumulator::new();
        assert!(matches!(
            acc.ensure_not_degenerate(),
            Err(RiskSieveError::DegenerateWeights)
        ));
        assert_eq!(acc.range(), None);
        assert_eq!(acc.effective_sample_size(), 0.0);
    }

    #[test]
    fn all_zero_weights_are_degenerate() {
        let mut acc = WeightAccumulator::new();
        acc.update(weight(0.0));
        acc.update(weight(0.0));
        assert!(matches!(
            acc.ensure_not_degenerate(),
            Err(RiskSieveError::DegenerateWeights)
        ));
    }

    #[test]
    fn a_later_positive_weight_resolves_degeneracy() {
        let mut acc = WeightAccumulator::new();
        acc.update(weight(0.0));
        assert!(acc.ensure_not_degenerate().is_err());
        acc.update(weight(1.0));
        assert!(acc.ensure_not_degenerate().is_ok());
    }

    #[test]
    fn constant_weights_give_ess_equal_to_count() {
        let mut acc = WeightAccumulator::new();
        for _ in 0..10 {
            acc.update(weight(1.0));
        }
        assert_eq!(acc.sum(), 10.0);
        assert_eq!(acc.sum_of_squares(), 10.0);
        assert_eq!(acc.effective_sample_size(), 10.0);
        assert_eq!(acc.mean(), 1.0);
    }

    #[test]
    fn tracks_min_and_max() {
        let mut acc = WeightAccumulator::new();
        for &v in &[2.0, 0.5, 5.0, 1.0] {
            acc.update(weight(v));
        }
        assert_eq!(acc.range(), Some((0.5, 5.0)));
    }

    #[test]
    fn extreme_but_finite_weights_stay_finite() {
        // AGENTS.md section 9.1: "extreme but finite importance weights."
        // 1e100 squared is 1e200, still comfortably inside f64's range.
        let mut acc = WeightAccumulator::new();
        acc.update(weight(1.0e100));
        acc.update(weight(1.0e100));
        assert!(acc.sum().is_finite());
        assert!(acc.sum_of_squares().is_finite());
        assert!(acc.effective_sample_size().is_finite());
    }

    #[test]
    fn matches_naive_sum_for_well_conditioned_weights() {
        let mut acc = WeightAccumulator::new();
        let values = [0.1, 0.2, 0.3, 0.4, 1.5];
        for &v in &values {
            acc.update(weight(v));
        }
        let naive: f64 = values.iter().sum();
        assert!((acc.sum() - naive).abs() < 1e-12);
    }
}
