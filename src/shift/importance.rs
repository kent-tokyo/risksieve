//! Incremental importance-weight validation and summary diagnostics.
//!
//! AGENTS.md Milestone 6 requires: non-negative finite weight validation
//! (delegated to [`NonNegative`]); diagnostics for the sum of weights,
//! sum of squared weights, effective sample size, minimum, and maximum;
//! and explicit failure for an all-zero or otherwise numerically
//! degenerate weight sequence, rather than a silent `NaN` or `0/0`.

use crate::error::RiskSieveError;
use crate::numerics::summation::kahan_sum;
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

    /// Folds in one more weight. `weight` is already validated
    /// non-negative and finite by construction (`NonNegative`), but the
    /// *accumulated* state can still overflow even from finite inputs
    /// (for example two weights near `f64::MAX`): this squares the
    /// weight, adds it to a running sum, and adds it to a running sum of
    /// squares, any of which can reach `+infinity` from finite operands.
    ///
    /// Every candidate value is computed into a local first and checked
    /// finite before anything is committed, so a rejected update leaves
    /// `self` byte-for-byte unchanged -- a caller can safely retry with a
    /// different weight, or discard the accumulator, without first
    /// having to know whether the failed call already mutated it.
    ///
    /// # Errors
    ///
    /// [`RiskSieveError::NumericalOverflow`] if the weight's square, the
    /// running sum, the running sum of squares, or the effective sample
    /// size they would produce is non-finite.
    pub fn update(&mut self, weight: NonNegative) -> Result<(), RiskSieveError> {
        let w = weight.get();

        let squared = w * w;
        if !squared.is_finite() {
            return Err(RiskSieveError::NumericalOverflow {
                operation: "WeightAccumulator::update: weight squared overflowed",
            });
        }

        let adjusted = w - self.sum_compensation;
        let candidate_sum = self.sum + adjusted;
        if !candidate_sum.is_finite() {
            return Err(RiskSieveError::NumericalOverflow {
                operation: "WeightAccumulator::update: running sum overflowed",
            });
        }
        let candidate_sum_compensation = (candidate_sum - self.sum) - adjusted;

        let adjusted_sq = squared - self.sum_of_squares_compensation;
        let candidate_sum_sq = self.sum_of_squares + adjusted_sq;
        if !candidate_sum_sq.is_finite() {
            return Err(RiskSieveError::NumericalOverflow {
                operation: "WeightAccumulator::update: running sum of squares overflowed",
            });
        }
        let candidate_sum_sq_compensation = (candidate_sum_sq - self.sum_of_squares) - adjusted_sq;

        let candidate_ess = if candidate_sum_sq == 0.0 {
            0.0
        } else {
            candidate_sum * candidate_sum / candidate_sum_sq
        };
        if !candidate_ess.is_finite() {
            return Err(RiskSieveError::NumericalOverflow {
                operation: "WeightAccumulator::update: effective sample size overflowed",
            });
        }

        self.sum = candidate_sum;
        self.sum_compensation = candidate_sum_compensation;
        self.sum_of_squares = candidate_sum_sq;
        self.sum_of_squares_compensation = candidate_sum_sq_compensation;
        self.min = self.min.min(w);
        self.max = self.max.max(w);
        self.count += 1;
        Ok(())
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

/// A batch, overflow-tolerant weight summary, for callers whose actual
/// guarantee computation already normalizes weights independently and so
/// must not be blocked by a diagnostic-only overflow --
/// `selective::mdr::certify_weighted`'s calibration weight diagnostics,
/// for example, since `selective::evalue_weighted::weighted_risk_adjusted_evalue`
/// itself normalizes by the shared maximum weight and stays exact
/// regardless of what this reports.
///
/// Unlike [`WeightAccumulator::update`], this never rejects an input: it
/// normalizes every weight by their shared maximum first (the same trick
/// the e-value construction above uses), computes `effective_sample_size`
/// from the normalized values (Kish's ESS is exactly scale-invariant, so
/// this is not an approximation), and reports `sum`/`sum_of_squares` at
/// the original raw scale only when that raw value is itself
/// representable in `f64` -- flagging the two overflow cases separately,
/// so a `None` here always means "overflowed", never "not computed"
/// (unlike a plain `Option<f64>` that silently conflated the two before,
/// see AGENTS.md's certificate serde-safety requirement and
/// `certificate::EValue`, which exists for the same reason).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightSummary {
    /// The raw-scale sum of weights, or `None` if it overflowed.
    pub sum: Option<f64>,
    /// Whether `sum` overflowed to non-finite at raw scale.
    pub sum_overflowed: bool,
    /// The raw-scale sum of squared weights, or `None` if it overflowed.
    pub sum_of_squares: Option<f64>,
    /// Whether `sum_of_squares` overflowed to non-finite at raw scale.
    pub sum_of_squares_overflowed: bool,
    /// Kish's effective sample size, computed scale-invariantly so it is
    /// exact even when `sum`/`sum_of_squares` themselves overflow.
    pub effective_sample_size: f64,
    /// `(min, max)` of the raw weights, or `None` for an empty slice.
    /// Individual weights are already validated finite by
    /// `NonNegative`, so this never overflows.
    pub range: Option<(f64, f64)>,
}

impl WeightSummary {
    /// Computes a [`WeightSummary`] for a batch of weights known ahead
    /// of time (unlike [`WeightAccumulator`], which folds them in one at
    /// a time as they arrive).
    pub fn compute(weights: &[NonNegative]) -> Self {
        if weights.is_empty() {
            return Self {
                sum: None,
                sum_overflowed: false,
                sum_of_squares: None,
                sum_of_squares_overflowed: false,
                effective_sample_size: 0.0,
                range: None,
            };
        }

        let values: Vec<f64> = weights.iter().map(|w| w.get()).collect();
        let range = Some((
            values.iter().copied().fold(f64::INFINITY, f64::min),
            values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        ));

        let scale = values.iter().copied().fold(0.0_f64, f64::max);
        if scale == 0.0 {
            // Every weight is exactly zero: nothing to normalize, and no
            // risk of overflow either.
            return Self {
                sum: Some(0.0),
                sum_overflowed: false,
                sum_of_squares: Some(0.0),
                sum_of_squares_overflowed: false,
                effective_sample_size: 0.0,
                range,
            };
        }

        // Canonical order before summing, matching the crate's
        // established fix for tied-value permutation dependence
        // (`evalue.rs`, `evalue_weighted.rs`): a diagnostic value should
        // not depend on the caller's input order any more than the
        // e-value itself does. Squaring preserves order for these
        // non-negative normalized values, so no separate sort is needed
        // before summing the squares.
        let mut normalized: Vec<f64> = values.iter().map(|&w| w / scale).collect();
        normalized.sort_by(f64::total_cmp);
        let normalized_sum = kahan_sum(normalized.iter().copied());
        let normalized_sum_of_squares = kahan_sum(normalized.iter().map(|u| u * u));

        let effective_sample_size = if normalized_sum_of_squares == 0.0 {
            0.0
        } else {
            normalized_sum * normalized_sum / normalized_sum_of_squares
        };

        let raw_sum = scale * normalized_sum;
        let sum_overflowed = !raw_sum.is_finite();
        let raw_sum_of_squares = scale * scale * normalized_sum_of_squares;
        let sum_of_squares_overflowed = !raw_sum_of_squares.is_finite();

        Self {
            sum: (!sum_overflowed).then_some(raw_sum),
            sum_overflowed,
            sum_of_squares: (!sum_of_squares_overflowed).then_some(raw_sum_of_squares),
            sum_of_squares_overflowed,
            effective_sample_size,
            range,
        }
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
        acc.update(weight(0.0)).unwrap();
        acc.update(weight(0.0)).unwrap();
        assert!(matches!(
            acc.ensure_not_degenerate(),
            Err(RiskSieveError::DegenerateWeights)
        ));
    }

    #[test]
    fn a_later_positive_weight_resolves_degeneracy() {
        let mut acc = WeightAccumulator::new();
        acc.update(weight(0.0)).unwrap();
        assert!(acc.ensure_not_degenerate().is_err());
        acc.update(weight(1.0)).unwrap();
        assert!(acc.ensure_not_degenerate().is_ok());
    }

    #[test]
    fn constant_weights_give_ess_equal_to_count() {
        let mut acc = WeightAccumulator::new();
        for _ in 0..10 {
            acc.update(weight(1.0)).unwrap();
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
            acc.update(weight(v)).unwrap();
        }
        assert_eq!(acc.range(), Some((0.5, 5.0)));
    }

    #[test]
    fn extreme_but_finite_weights_stay_finite() {
        // AGENTS.md section 9.1: "extreme but finite importance weights."
        // 1e100 squared is 1e200, still comfortably inside f64's range.
        let mut acc = WeightAccumulator::new();
        acc.update(weight(1.0e100)).unwrap();
        acc.update(weight(1.0e100)).unwrap();
        assert!(acc.sum().is_finite());
        assert!(acc.sum_of_squares().is_finite());
        assert!(acc.effective_sample_size().is_finite());
    }

    #[test]
    fn matches_naive_sum_for_well_conditioned_weights() {
        let mut acc = WeightAccumulator::new();
        let values = [0.1, 0.2, 0.3, 0.4, 1.5];
        for &v in &values {
            acc.update(weight(v)).unwrap();
        }
        let naive: f64 = values.iter().sum();
        assert!((acc.sum() - naive).abs() < 1e-12);
    }

    #[test]
    fn update_rejects_a_weight_whose_square_overflows() {
        let mut acc = WeightAccumulator::new();
        let before = acc;
        assert!(matches!(
            acc.update(weight(f64::MAX)),
            Err(RiskSieveError::NumericalOverflow { .. })
        ));
        assert_eq!(acc, before, "a rejected update must not mutate state");
    }

    #[test]
    fn update_rejects_a_sum_that_overflows_independently_of_the_square_check() {
        // `f64::MAX`'s square root (~1.34e154) is the largest weight
        // whose own square stays finite, and it is utterly negligible
        // next to `f64::MAX` itself (~1.8e308) -- so a sum can only be
        // pushed over the edge by a square-safe weight if it was already
        // sitting at `f64::MAX` beforehand. Constructing that state
        // directly (rather than building it up through `update`, which
        // no square-safe sequence of calls could ever reach) isolates
        // the sum-overflow check from the square-overflow check.
        let mut acc = WeightAccumulator {
            count: 1,
            sum: f64::MAX,
            sum_compensation: 0.0,
            sum_of_squares: 1.0,
            sum_of_squares_compensation: 0.0,
            min: f64::MAX,
            max: f64::MAX,
        };
        let before = acc;
        assert!(matches!(
            acc.update(weight(1.0)),
            Err(RiskSieveError::NumericalOverflow { .. })
        ));
        assert_eq!(acc, before, "a rejected update must not mutate state");
    }

    #[test]
    fn a_rejected_update_does_not_prevent_later_valid_updates() {
        let mut acc = WeightAccumulator::new();
        acc.update(weight(1.0)).unwrap();
        assert!(acc.update(weight(f64::MAX)).is_err());
        acc.update(weight(2.0)).unwrap();
        assert_eq!(acc.sum(), 3.0);
        assert_eq!(acc.count(), 2);
    }

    #[test]
    fn weight_summary_of_empty_slice_is_all_none() {
        let summary = WeightSummary::compute(&[]);
        assert_eq!(summary.sum, None);
        assert_eq!(summary.sum_of_squares, None);
        assert!(!summary.sum_overflowed);
        assert!(!summary.sum_of_squares_overflowed);
        assert_eq!(summary.effective_sample_size, 0.0);
        assert_eq!(summary.range, None);
    }

    #[test]
    fn weight_summary_single_f64_max_keeps_sum_but_not_sum_of_squares() {
        // A single f64::MAX weight has a representable raw sum
        // (scale * normalized_sum = f64::MAX * 1.0), but its raw sum of
        // squares (f64::MAX^2) is not -- overflow in one field must not
        // force the other to `None` too.
        let summary = WeightSummary::compute(&[weight(f64::MAX)]);
        assert_eq!(summary.sum, Some(f64::MAX));
        assert!(!summary.sum_overflowed);
        assert_eq!(summary.sum_of_squares, None);
        assert!(summary.sum_of_squares_overflowed);
        assert_eq!(summary.effective_sample_size, 1.0);
        assert_eq!(summary.range, Some((f64::MAX, f64::MAX)));
    }

    #[test]
    fn weight_summary_two_f64_max_overflows_both_raw_fields() {
        let summary = WeightSummary::compute(&[weight(f64::MAX), weight(f64::MAX)]);
        assert_eq!(summary.sum, None);
        assert!(summary.sum_overflowed);
        assert_eq!(summary.sum_of_squares, None);
        assert!(summary.sum_of_squares_overflowed);
        // Kish's ESS for two equal weights is exactly 2, computed from
        // the normalized values regardless of the raw-scale overflow.
        assert_eq!(summary.effective_sample_size, 2.0);
    }

    #[test]
    fn weight_summary_extreme_ratio_overflows_only_sum_of_squares() {
        // The true sum (~1e300) stays representable, but the true sum of
        // squares (~1e600) does not -- these must be flagged
        // independently, not conflated into one shared overflow bit.
        let summary = WeightSummary::compute(&[weight(1e-300), weight(1e300)]);
        assert!(!summary.sum_overflowed);
        assert!(summary.sum.unwrap().is_finite());
        assert_eq!(summary.sum_of_squares, None);
        assert!(summary.sum_of_squares_overflowed);
    }

    #[test]
    fn weight_summary_zero_weights_alongside_f64_max_stay_exact() {
        let summary = WeightSummary::compute(&[weight(0.0), weight(0.0), weight(f64::MAX)]);
        assert_eq!(summary.sum, Some(f64::MAX));
        assert!(!summary.sum_overflowed);
        assert_eq!(summary.sum_of_squares, None);
        assert!(summary.sum_of_squares_overflowed);
        assert_eq!(summary.effective_sample_size, 1.0);
        assert_eq!(summary.range, Some((0.0, f64::MAX)));
    }

    #[test]
    fn weight_summary_all_zero_weights_is_exact_not_overflowed() {
        let summary = WeightSummary::compute(&[weight(0.0), weight(0.0)]);
        assert_eq!(summary.sum, Some(0.0));
        assert!(!summary.sum_overflowed);
        assert_eq!(summary.sum_of_squares, Some(0.0));
        assert!(!summary.sum_of_squares_overflowed);
        assert_eq!(summary.effective_sample_size, 0.0);
    }

    #[test]
    fn weight_summary_matches_naive_computation_for_well_conditioned_weights() {
        let values = [0.1, 0.2, 0.3, 0.4, 1.5];
        let summary =
            WeightSummary::compute(&values.iter().map(|&v| weight(v)).collect::<Vec<_>>());
        let naive_sum: f64 = values.iter().sum();
        let naive_sum_sq: f64 = values.iter().map(|v| v * v).sum();
        assert!((summary.sum.unwrap() - naive_sum).abs() < 1e-12);
        assert!((summary.sum_of_squares.unwrap() - naive_sum_sq).abs() < 1e-12);
        assert!(!summary.sum_overflowed);
        assert!(!summary.sum_of_squares_overflowed);
    }
}
