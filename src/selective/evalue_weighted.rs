//! Weighted risk-adjusted e-values under covariate shift (Equation 6.1,
//! Assumption 6.1, Theorem 6.2, Theorem 6.4).
//!
//! Bai and Jin (2026), *Conformal Selective Prediction with General Risk
//! Control*, arXiv:2603.24704, Section 6 extends [`super::evalue`]'s
//! Equation 4.1 to a test distribution that differs from the calibration
//! distribution by a known or estimated importance weight.
//!
//! **Assumption 6.1:** `{(X_i,Y_i)}_{i=1}^n` are i.i.d. from `P`
//! (calibration), `(X_{n+1},Y_{n+1})` is drawn from `Q` (test), and
//! `dQ/dP(x,y) = w(x)` for a known or estimable weight function
//! `w: X -> R^+`. Weights carry no normalization requirement (they need
//! not sum to `n`, to `1`, or to anything else -- see "Uniform scale
//! invariance" below).
//!
//! **Equation 6.1** (the weighted e-value):
//!
//! ```text
//! E_{gamma,n+1} = inf_{l in [0,1]} { 1{s(X_{n+1}) <= t_gamma(l)} * sum_{i=1}^{n+1} w_i
//!                 / ( sum_{i=1}^n w_i * L_i * 1{s(X_i) <= t_gamma(l)}
//!                     + w_{n+1} * l * 1{s(X_{n+1}) <= t_gamma(l)} ) }
//!
//! t_gamma(l) = max{ t in M : F(t;l) <= gamma }
//! F(t;l) = ( sum_{i=1}^n w_i*L_i*1{s(X_i)<=t} + w_{n+1}*l*1{s(X_{n+1})<=t} )
//!           / sum_{i=1}^{n+1} w_i
//! ```
//!
//! where `w_i = w(X_i)`, `M = {s(X_i)}_{i=1}^{n+1}`, and `E_{gamma,n+1}
//! := 0` when `inf_{l} t_gamma(l) = -infinity` (no threshold is ever
//! feasible).
//!
//! **Theorem 6.2:** under Assumption 6.1 with `w` known exactly, for any
//! fixed `gamma in (0,1)`, `E_Q[L_{n+1} * E_{gamma,n+1}] <= 1` -- the same
//! domain Theorem 4.2 states for the unweighted e-value (Section 6 does
//! not widen it, unlike Section 5's SDR extension). [`super::mdr::certify_weighted`]
//! thresholds this e-value at `1/alpha` exactly as unweighted MDR does,
//! yielding `E_Q[L_{n+1} * psi_{n+1}] <= alpha`.
//!
//! **Theorem 6.4** (estimated weights): if `w` is unknown and instead
//! estimated by a sequence `\bar{w}_n` trained *independent* of the
//! calibration data used to compute the e-value, with
//! `||\bar{w}_n - w||_{L2(P_X)} = o_P(1)` (consistency) plus a mild
//! regularity condition, then `limsup_{n->infinity} MDR_n <= alpha` --
//! an asymptotic statement, not a finite-sample one. This is exactly
//! what [`crate::guarantee::ImportanceWeightSource::Estimated`]'s
//! `training_data_separate_from_calibration` field already asks the
//! caller to declare (see [`super::mdr::certify_weighted`]'s module
//! docs for how this determines the returned `GuaranteeKind`).
//!
//! ## Correspondence to `Tian-Bai/SCoRE`'s `SCoRE_MDR_w`
//!
//! `SCoRE_MDR_w` (`SCoRE/SCoRE.py`, commit
//! `401b7caf6d030825ff67e8f08e44ba15ee8c94af`, package version `0.1.1`)
//! implements a *decision-only* shortcut for this equation -- like the
//! unweighted `SCoRE_MDR`, it never computes `E_{gamma,n+1}` itself, only
//! the thresholded deploy/abstain decision, via a closed form valid for
//! `gamma <= alpha` (with an extra condition checked for `gamma > alpha`,
//! mirroring Proposition 4.4). There is no official `SCoRE_MDR_w_bf`
//! (weighted brute-force) counterpart to cross-check the actual e-value
//! against; this module was derived directly from Equation 6.1 rather
//! than from the shortcut, and is cross-checked against `SCoRE_MDR_w`'s
//! *decisions* (not its e-values, which it does not expose) in the oracle
//! fixture.
//!
//! | Equation 6.1 | `SCoRE_MDR_w` | This module |
//! |---|---|---|
//! | `w_i` (calibration) | `wcalib` | `calibration_weights: &[NonNegative]` |
//! | `w_{n+1}` (test point) | `wtest[i_itr]` | `test_weight: NonNegative` |
//! | `sum_i w_i*L_i*1{s(X_i)<=t}` | `np.sum(wcalib*Lcalib*(Scalib<=t))` | `weighted_base_sum[j]` (grouped by score, Kahan-summed in canonical order) |
//! | `sum_{i=1}^{n+1} w_i` (total weight -- constant in `t`) | `wtest[i_itr] + calib_w_sum` | `total_weight` |
//! | the `l` solving `F(t;l) = gamma` for a given `t` | not computed (shortcut avoids it) | `(gamma_scaled - weighted_base_sum[j]) / test_weight`, clamped to `[0,1]`, skipped entirely when `test_weight == 0` (see "Zero test weight" below) |
//! | `E_{gamma,n+1}` | not computed (decision-only shortcut) | the returned [`WeightedEValueOutcome::value`], an [`EValue`] (see its docs for why this is not simply a [`crate::probability::NonNegative`]) |
//!
//! ## Why this reuses `evalue.rs`'s structure rather than `mdr_w`'s shortcut
//!
//! Substituting `w_i * L_i` for `L_i` everywhere and `sum_{i=1}^{n+1} w_i`
//! for the unweighted `(n+1)` turns Equation 6.1 into exactly Equation
//! 4.1's shape: `F(t;l)` is still non-decreasing in both `t` and `l`
//! (every term is an indicator times a non-negative quantity, since
//! `w_i >= 0` and `L_i in [0,1]`), so the same breakpoint-enumeration
//! argument [`super::evalue`]'s module docs give applies verbatim -- the
//! infimum is attained at `l in {0, 1}` or at the `l` where a threshold's
//! feasibility constraint turns from satisfied to violated. This module
//! is a parallel, independent implementation (not a refactor of
//! `evalue.rs` to accept weights), per this PR's explicit instruction not
//! to fold the two together if doing so risks changing either one's
//! numerical behavior.
//!
//! ## Uniform scale invariance
//!
//! Multiplying every weight (all calibration weights *and* the test
//! weight, by the same positive constant `c`) leaves `E_{gamma,n+1}`
//! unchanged: `F(t;l)`'s numerator and denominator both scale by `c`,
//! so `F(t;l)` itself -- and therefore `t_gamma(l)` -- is unchanged;
//! the outer objective's numerator (`indicator * sum w_i`) and
//! denominator (`weighted_base_sum + w_{n+1}*l*indicator`) also both
//! scale by `c`, cancelling in the ratio. This does *not* hold for
//! rescaling only a subset of the weights (for example calibration
//! weights alone, leaving the test weight fixed) -- see
//! `tests::uniform_weight_rescale_leaves_evalue_unchanged` and the
//! property test in `mdr.rs`.
//!
//! This implementation actively *relies on* this invariance for
//! numerical safety, not just correctness: every weight (calibration and
//! test alike) is normalized by their shared maximum before any other
//! computation, so every value used downstream lies in `[0, 1]`.
//! Computing at the caller's raw scale instead can overflow `f64` for
//! ordinary-looking finite inputs (for example two weights near
//! `f64::MAX`), which previously produced a spurious
//! `EValue::PositiveInfinity` for what is, once the shared scale is
//! factored out, a genuinely finite e-value -- see
//! `tests::huge_but_finite_weights_do_not_spuriously_overflow_to_infinity`.
//!
//! ## Zero test weight
//!
//! When `test_weight == 0`, the `l`-term (`w_{n+1}*l*indicator`) vanishes
//! from `F(t;l)` for every `l`, so `t_gamma(l)` is identical for every
//! `l in [0,1]` and the per-threshold breakpoint (which would otherwise
//! divide by `test_weight`) carries no information -- `l in {0,1}` alone
//! already cover the (degenerate, constant) range. This module detects
//! `test_weight == 0` and skips generating that breakpoint rather than
//! dividing by zero.
//!
//! ## Degenerate combined weights
//!
//! Only the *combined* calibration-plus-test weight sum needs to be
//! checked for degeneracy (unlike calibration weights alone in
//! [`crate::shift::importance::WeightAccumulator`]'s existing use): the
//! shared normalizing constant `sum_{i=1}^{n+1} w_i` is what every ratio
//! in Equation 6.1 divides by. If every calibration weight *and* the test
//! weight are exactly zero, this module returns
//! [`RiskSieveError::DegenerateWeights`] rather than producing `0/0`.
//! A subset being zero (some calibration weights, or the test weight
//! alone) is not degenerate, provided the combined sum is positive.

use crate::error::RiskSieveError;
use crate::numerics::summation::kahan_sum;
use crate::probability::{ClosedUnitInterval, NonNegative, OpenUnitInterval, check_finite};

/// A weighted risk-adjusted e-value, which -- unlike the unweighted
/// construction in [`super::evalue`] -- can be mathematically
/// `+infinity`, not merely large.
///
/// This happens only when `test_weight == 0` (so `l` has no effect on
/// `F(t;l)`, per the module docs' "Zero test weight" section) *and* the
/// weighted calibration loss at every feasible threshold is exactly `0`
/// (every calibration point below the threshold has zero loss, zero
/// weight, or both). The combined calibration-plus-test weight is
/// nonzero in this case (otherwise [`RiskSieveError::DegenerateWeights`]
/// is returned instead), so this is a genuine value of the infimum in
/// Equation 6.1, not a numerical error -- deploying is still the correct
/// decision (thresholding at `1/alpha` for any `alpha < 1` deploys
/// unconditionally when the e-value is unbounded), and the official
/// `SCoRE_MDR_w` shortcut deploys on the fixture exercising this case
/// too (`tests/fixtures/score_mdr_w_v0_1_1.json`'s `zero_weights` case).
/// Clamping this to a large finite value or silently rejecting it would
/// misstate the guarantee (AGENTS.md section 8: "never silently saturate
/// an e-value").
///
/// Defined in [`crate::certificate`] (re-exported here for backward
/// compatibility) so [`crate::certificate::Diagnostics::risk_adjusted_evalue`]
/// can use it without that foundational module depending on this
/// Milestone-6-specific one.
pub use crate::certificate::EValue;

fn normalize_zero(x: f64) -> f64 {
    if x == 0.0 { 0.0 } else { x }
}

/// See [`super::evalue`]'s identically-named function for the rationale;
/// the same conservative-rounding argument applies unchanged here.
fn feasibility_epsilon(rhs: f64) -> f64 {
    rhs.abs().max(1.0) * 8.0 * f64::EPSILON
}

/// The result of evaluating Equation 6.1's weighted e-value.
///
/// Mirrors [`super::evalue::EValueOutcome`]'s shape, with `value` widened
/// to [`EValue`] to represent the genuinely-possible `+infinity` case
/// (see that type's docs) -- reusing `EValueOutcome` itself is not
/// possible since its `value` field is fixed to `NonNegative`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightedEValueOutcome {
    /// The computed weighted risk-adjusted e-value.
    pub value: EValue,
    /// `false` when the value minimizing Equation 6.1 comes from the
    /// case where *no* threshold in `M` satisfies `F(t;l) <= gamma` for
    /// any `l` -- the same distinction
    /// [`super::evalue::EValueOutcome::feasible_threshold_found`] makes,
    /// orthogonal to whether `value` is finite or `PositiveInfinity`.
    pub feasible_threshold_found: bool,
}

/// Computes the weighted risk-adjusted e-value `E_{gamma,n+1}` (Equation
/// 6.1) for one test point against a weighted calibration set.
///
/// `calibration_losses[i]`, `calibration_scores[i]`, and
/// `calibration_weights[i]` must all correspond to the same calibration
/// point `i`; all three slices must have equal, nonzero length.
///
/// # Errors
///
/// - [`RiskSieveError::AssumptionMismatch`] if the three calibration
///   slices have different lengths.
/// - [`RiskSieveError::EmptyCalibrationSet`] if they are empty.
/// - [`RiskSieveError::NonFiniteValue`] if `test_score` or any
///   calibration score is NaN or infinite.
/// - [`RiskSieveError::DegenerateWeights`] if every calibration weight
///   *and* `test_weight` are exactly zero (see the module docs).
pub fn weighted_risk_adjusted_evalue(
    calibration_losses: &[ClosedUnitInterval],
    calibration_scores: &[f64],
    calibration_weights: &[NonNegative],
    test_score: f64,
    test_weight: NonNegative,
    gamma: OpenUnitInterval,
) -> Result<WeightedEValueOutcome, RiskSieveError> {
    if calibration_losses.len() != calibration_scores.len()
        || calibration_losses.len() != calibration_weights.len()
    {
        return Err(RiskSieveError::AssumptionMismatch {
            detail: format!(
                "calibration_losses ({}), calibration_scores ({}), and \
                 calibration_weights ({}) must have equal length",
                calibration_losses.len(),
                calibration_scores.len(),
                calibration_weights.len(),
            ),
        });
    }
    if calibration_losses.is_empty() {
        return Err(RiskSieveError::EmptyCalibrationSet);
    }
    check_finite("test_score", test_score)?;
    for &score in calibration_scores {
        check_finite("calibration_scores", score)?;
    }

    // Normalize every weight (calibration and test alike) by their
    // shared maximum *before* computing anything else. Equation 6.1 is
    // invariant to a uniform positive rescaling of every weight together
    // (see the module docs, "Uniform scale invariance", and
    // `uniform_weight_rescale_leaves_evalue_unchanged`), so this changes
    // nothing mathematically -- but it guarantees every weight used from
    // here on is in `[0, 1]`, with at least one exactly `1.0`, so no
    // finite input can make `total_weight`, a weighted loss, or
    // `gamma_scaled` overflow to `+infinity`. Computing at the raw input
    // scale instead can overflow for finite, non-adversarial-looking
    // inputs (for example two `NonNegative` weights near `f64::MAX`),
    // silently producing `EValue::PositiveInfinity` for what is
    // mathematically a finite e-value once the shared scale cancels --
    // see `huge_but_finite_weights_do_not_spuriously_overflow_to_infinity`.
    let max_weight = calibration_weights
        .iter()
        .map(|w| w.get())
        .fold(test_weight.get(), f64::max);
    if max_weight == 0.0 {
        return Err(RiskSieveError::DegenerateWeights);
    }

    let test_score = normalize_zero(test_score);
    let test_weight = test_weight.get() / max_weight;

    // Grouped by `(score, weighted_loss)`, not score alone -- see
    // `evalue.rs`'s identical fix and its regression test for why a
    // tied group's summation order must not depend on caller input
    // order.
    let mut entries: Vec<(f64, f64)> = calibration_scores
        .iter()
        .zip(calibration_losses.iter())
        .zip(calibration_weights.iter())
        .map(|((&score, &loss), &weight)| {
            (
                normalize_zero(score),
                (weight.get() / max_weight) * loss.get(),
            )
        })
        .collect();
    entries.push((test_score, 0.0));
    entries.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));

    // Sum of the *normalized* weights: bounded by `calibration_weights.len() + 1`
    // (every term is now in `[0, 1]`), so this can never overflow the way
    // summing raw, unnormalized weights could. Sorted by value via
    // `total_cmp` before summing -- the same "canonical order, not input
    // order" fix already applied to `evalue.rs` and `coupled.rs`'s tied
    // score groups -- so the sum depends only on the multiset of
    // (normalized) weights, never on the caller's input order. Summing
    // in caller-supplied order instead is a latent version of that same
    // bug: normalizing here adds a division's worth of extra rounding to
    // each term, which is enough to make an already input-order-sensitive
    // Kahan sum disagree at the last bit between permutations (caught by
    // `construction_is_permutation_invariant`).
    let mut normalized_calibration_weights: Vec<f64> = calibration_weights
        .iter()
        .map(|w| w.get() / max_weight)
        .collect();
    normalized_calibration_weights.sort_by(f64::total_cmp);
    let total_weight = kahan_sum(normalized_calibration_weights.iter().copied()) + test_weight;

    let mut values: Vec<f64> = Vec::new();
    let mut per_value_sum: Vec<f64> = Vec::new();
    let mut group_start = 0;
    for i in 0..=entries.len() {
        let at_boundary = i == entries.len() || entries[i].0 != entries[group_start].0;
        if at_boundary && i > group_start {
            values.push(entries[group_start].0);
            per_value_sum.push(kahan_sum(entries[group_start..i].iter().map(|&(_, wl)| wl)));
            group_start = i;
        }
    }

    // Compensated running prefix sum (AGENTS.md section 8): base_sum[j]
    // is the total weighted calibration loss at scores <= values[j].
    let mut base_sum = Vec::with_capacity(values.len());
    let mut running = 0.0_f64;
    let mut compensation = 0.0_f64;
    for &value in &per_value_sum {
        let adjusted = value - compensation;
        let new_running = running + adjusted;
        compensation = (new_running - running) - adjusted;
        running = new_running;
        base_sum.push(running);
    }

    let test_below: Vec<bool> = values.iter().map(|&v| test_score <= v).collect();
    let gamma_scaled = gamma.get() * total_weight;
    let epsilon = feasibility_epsilon(gamma_scaled);

    let largest_feasible_index = |ell: f64| -> Option<usize> {
        (0..base_sum.len()).rev().find(|&j| {
            let contribution = base_sum[j]
                + if test_below[j] {
                    test_weight * ell
                } else {
                    0.0
                };
            contribution <= gamma_scaled + epsilon
        })
    };

    let objective_at = |ell: f64, j: Option<usize>| -> f64 {
        match j {
            Some(j) if test_below[j] => total_weight / (base_sum[j] + test_weight * ell),
            _ => 0.0,
        }
    };

    // Breakpoint candidates: see the module docs ("Why this reuses
    // evalue.rs's structure") for why the infimum is attained at one of
    // these. The per-threshold breakpoint is skipped when
    // `test_weight == 0` (see "Zero test weight" in the module docs).
    let mut candidates: Vec<f64> = vec![0.0, 1.0];
    if test_weight > 0.0 {
        for (j, &below) in test_below.iter().enumerate() {
            if below {
                candidates.push(((gamma_scaled - base_sum[j]) / test_weight).clamp(0.0, 1.0));
            }
        }
    }

    // Tracked as `Option`, not a `f64::INFINITY` sentinel compared via
    // `<`: unlike `evalue.rs` (whose objective is provably always
    // finite, so a finite first candidate always overwrites the
    // sentinel), a candidate's own value can legitimately *be*
    // `+infinity` here (see `EValue`'s docs) -- `infinity < infinity` is
    // `false`, which would silently skip recording that a candidate was
    // ever evaluated at all if a plain sentinel comparison were reused.
    let mut best: Option<(f64, bool)> = None;
    for &ell in &candidates {
        let j = largest_feasible_index(ell);
        let value = objective_at(ell, j);
        let feasible_here = j.is_some();
        best = Some(match best {
            None => (value, feasible_here),
            Some((current_value, _)) if value < current_value => (value, feasible_here),
            Some(existing) => existing,
        });
    }
    let (best_value, best_feasible) = best.expect("candidates always contains at least [0.0, 1.0]");

    let value = if best_value.is_infinite() {
        EValue::PositiveInfinity
    } else {
        EValue::Finite(NonNegative::new(
            "weighted_risk_adjusted_evalue",
            best_value,
        )?)
    };

    Ok(WeightedEValueOutcome {
        value,
        feasible_threshold_found: best_feasible,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn losses(values: &[f64]) -> Vec<ClosedUnitInterval> {
        values
            .iter()
            .map(|&v| ClosedUnitInterval::new("loss", v).unwrap())
            .collect()
    }

    fn weights(values: &[f64]) -> Vec<NonNegative> {
        values
            .iter()
            .map(|&v| NonNegative::new("weight", v).unwrap())
            .collect()
    }

    fn weight(v: f64) -> NonNegative {
        NonNegative::new("weight", v).unwrap()
    }

    fn gamma(v: f64) -> OpenUnitInterval {
        OpenUnitInterval::new("gamma", v).unwrap()
    }

    #[test]
    fn rejects_mismatched_lengths() {
        let err = weighted_risk_adjusted_evalue(
            &losses(&[1.0]),
            &[0.0, 1.0],
            &weights(&[1.0]),
            0.5,
            weight(1.0),
            gamma(0.5),
        )
        .unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn rejects_empty_calibration() {
        let err =
            weighted_risk_adjusted_evalue(&[], &[], &[], 0.5, weight(1.0), gamma(0.5)).unwrap_err();
        assert!(matches!(err, RiskSieveError::EmptyCalibrationSet));
    }

    #[test]
    fn rejects_non_finite_test_score() {
        let err = weighted_risk_adjusted_evalue(
            &losses(&[1.0]),
            &[0.0],
            &weights(&[1.0]),
            f64::NAN,
            weight(1.0),
            gamma(0.5),
        )
        .unwrap_err();
        assert!(matches!(err, RiskSieveError::NonFiniteValue { .. }));
    }

    #[test]
    fn rejects_fully_degenerate_combined_weights() {
        let err = weighted_risk_adjusted_evalue(
            &losses(&[1.0, 0.0]),
            &[0.0, 1.0],
            &weights(&[0.0, 0.0]),
            0.5,
            weight(0.0),
            gamma(0.5),
        )
        .unwrap_err();
        assert!(matches!(err, RiskSieveError::DegenerateWeights));
    }

    #[test]
    fn all_zero_calibration_weight_with_positive_test_weight_is_not_degenerate() {
        // Only the *combined* sum matters; a zero-weighted calibration
        // set with a positive test weight is well-defined (matches
        // "all_zero_calibration_weight_only" in the module docs).
        let outcome = weighted_risk_adjusted_evalue(
            &losses(&[1.0, 1.0]),
            &[0.0, 1.0],
            &weights(&[0.0, 0.0]),
            0.0,
            weight(1.0),
            gamma(0.5),
        )
        .unwrap();
        assert!(outcome.value.as_f64().is_finite());
    }

    #[test]
    fn zero_test_weight_is_not_degenerate_and_does_not_panic() {
        let outcome = weighted_risk_adjusted_evalue(
            &losses(&[0.0, 1.0]),
            &[0.0, 1.0],
            &weights(&[1.0, 1.0]),
            0.5,
            weight(0.0),
            gamma(0.5),
        )
        .unwrap();
        assert!(outcome.value.as_f64().is_finite() && outcome.value.as_f64() >= 0.0);
    }

    /// Hand-traceable `EValue::PositiveInfinity` case, found while
    /// generating the oracle fixture (`zero_weights` in
    /// `tests/fixtures/score_mdr_w_v0_1_1.json`): calibration point 0 has
    /// the only positive loss (`1.0`) but zero weight, calibration point
    /// 1 has zero loss, and the test point's own weight is zero too. The
    /// weighted calibration loss is therefore exactly `0` at every
    /// threshold regardless of `l` (since `test_weight == 0` removes any
    /// `l`-dependence -- see the module docs), while the test point's own
    /// score (`0.0`) is `<=` the largest pooled score (`1.0`), so its
    /// indicator is `1` there. The true infimum is `total_weight / 0`, a
    /// genuine mathematical `+infinity`, not a numerical accident. The
    /// official `SCoRE_MDR_w` shortcut deploys on this exact input too
    /// (recorded in the fixture), consistent with an unbounded e-value.
    #[test]
    fn zero_test_weight_with_zero_weighted_loss_gives_positive_infinity() {
        let outcome = weighted_risk_adjusted_evalue(
            &losses(&[1.0, 0.0]),
            &[0.0, 1.0],
            &weights(&[0.0, 1.0]),
            0.0,
            weight(0.0),
            gamma(0.5),
        )
        .unwrap();
        assert_eq!(outcome.value, EValue::PositiveInfinity);
        assert!(outcome.feasible_threshold_found);

        // Deploys unconditionally, for any alpha < 1.
        assert!(
            outcome
                .value
                .clears_deployment_threshold(OpenUnitInterval::new("alpha", 0.999).unwrap())
        );
        assert!(
            outcome
                .value
                .clears_deployment_threshold(OpenUnitInterval::new("alpha", 0.001).unwrap())
        );
    }

    /// At `w_i = 1` for every calibration point and the test point,
    /// Equation 6.1 reduces algebraically to Equation 4.1 exactly (see
    /// the module docs): `sum w_i = n+1`, `weighted_base_sum = base_sum`.
    /// Cross-checked directly against `evalue::risk_adjusted_evalue`
    /// (not merely re-derived), including on the module's own two
    /// hand-traced fixtures from `tests/paper_score_mdr.rs`.
    #[test]
    fn all_weights_equal_to_one_matches_unweighted_hand_fixtures() {
        use crate::selective::evalue::risk_adjusted_evalue;

        let g = gamma(0.5);
        let weighted = weighted_risk_adjusted_evalue(
            &losses(&[1.0]),
            &[0.0],
            &weights(&[1.0]),
            1.0,
            weight(1.0),
            g,
        )
        .unwrap();
        let unweighted = risk_adjusted_evalue(&losses(&[1.0]), &[0.0], 1.0, g).unwrap();
        assert_eq!(weighted.value.as_f64(), unweighted.value.get());
        assert_eq!(weighted.value.as_f64(), 0.0);

        let weighted2 = weighted_risk_adjusted_evalue(
            &losses(&[0.0]),
            &[1.0],
            &weights(&[1.0]),
            0.0,
            weight(1.0),
            g,
        )
        .unwrap();
        let unweighted2 = risk_adjusted_evalue(&losses(&[0.0]), &[1.0], 0.0, g).unwrap();
        assert_eq!(weighted2.value.as_f64(), unweighted2.value.get());
        assert_eq!(weighted2.value.as_f64(), 2.0);
    }

    #[test]
    fn uniform_weight_rescale_leaves_evalue_unchanged() {
        let g = gamma(0.6);
        let base = weighted_risk_adjusted_evalue(
            &losses(&[0.3, 0.7, 0.1]),
            &[-1.0, 0.5, 2.0],
            &weights(&[2.0, 5.0, 1.0]),
            1.0,
            weight(3.0),
            g,
        )
        .unwrap();
        let rescaled = weighted_risk_adjusted_evalue(
            &losses(&[0.3, 0.7, 0.1]),
            &[-1.0, 0.5, 2.0],
            &weights(&[20.0, 50.0, 10.0]),
            1.0,
            weight(30.0),
            g,
        )
        .unwrap();
        assert_eq!(base.value.as_f64(), rescaled.value.as_f64());
    }

    #[test]
    fn non_uniform_rescale_of_calibration_only_can_change_the_evalue() {
        // Rescaling only calibration weights (leaving the test weight
        // fixed) is *not* covered by the uniform-scale-invariance
        // argument -- confirm it can actually move the result, so the
        // invariance property test above is not vacuous.
        let g = gamma(0.6);
        let base = weighted_risk_adjusted_evalue(
            &losses(&[0.3, 0.7, 0.1]),
            &[-1.0, 0.5, 2.0],
            &weights(&[2.0, 5.0, 1.0]),
            1.0,
            weight(3.0),
            g,
        )
        .unwrap();
        let calib_only_rescaled = weighted_risk_adjusted_evalue(
            &losses(&[0.3, 0.7, 0.1]),
            &[-1.0, 0.5, 2.0],
            &weights(&[20.0, 50.0, 10.0]),
            1.0,
            weight(3.0),
            g,
        )
        .unwrap();
        assert_ne!(base.value.as_f64(), calib_only_rescaled.value.as_f64());
    }

    /// Regression: computing at the raw weight scale, `total_weight =
    /// calibration_weight_sum + test_weight` overflows to `+infinity` for
    /// weights this large (`f64::MAX + f64::MAX`), which previously made
    /// this function return `EValue::PositiveInfinity` even though the
    /// true e-value is finite -- Equation 6.1's shared weight scale
    /// cancels in the ratio once normalized. Calibration loss `0` and a
    /// calibration score tied with the test score, weight `f64::MAX` on
    /// both sides, `gamma = 0.5`: normalizing by the shared max weight
    /// gives calibration weight `1.0` and test weight `1.0`, so
    /// `total_weight = 2.0`, `gamma_scaled = 1.0`, weighted calibration
    /// loss is `0` at every threshold, and the objective at `l = 1` is
    /// `2.0 / (0 + 1.0) = 2.0` -- the true infimum.
    #[test]
    fn huge_but_finite_weights_do_not_spuriously_overflow_to_infinity() {
        let outcome = weighted_risk_adjusted_evalue(
            &losses(&[0.0]),
            &[1.0],
            &weights(&[f64::MAX]),
            1.0,
            weight(f64::MAX),
            gamma(0.5),
        )
        .unwrap();
        assert_eq!(
            outcome.value,
            EValue::Finite(NonNegative::new("e", 2.0).unwrap())
        );
    }

    #[test]
    fn uniform_weight_rescale_near_f64_max_leaves_evalue_unchanged() {
        let g = gamma(0.6);
        let base = weighted_risk_adjusted_evalue(
            &losses(&[0.3, 0.7, 0.1]),
            &[-1.0, 0.5, 2.0],
            &weights(&[2.0, 5.0, 1.0]),
            1.0,
            weight(3.0),
            g,
        )
        .unwrap();
        let scale = 1e300;
        let rescaled = weighted_risk_adjusted_evalue(
            &losses(&[0.3, 0.7, 0.1]),
            &[-1.0, 0.5, 2.0],
            &weights(&[2.0 * scale, 5.0 * scale, 1.0 * scale]),
            1.0,
            weight(3.0 * scale),
            g,
        )
        .unwrap();
        assert_eq!(base.value.as_f64(), rescaled.value.as_f64());
    }

    #[test]
    fn subnormal_to_normal_uniform_rescale_leaves_evalue_unchanged() {
        let g = gamma(0.6);
        // Entirely subnormal-scale weights (all below `f64::MIN_POSITIVE`'s
        // normal-range boundary).
        let subnormal = weighted_risk_adjusted_evalue(
            &losses(&[0.3, 0.7, 0.1]),
            &[-1.0, 0.5, 2.0],
            &weights(&[2e-310, 5e-310, 1e-310]),
            1.0,
            weight(3e-310),
            g,
        )
        .unwrap();
        // Same ratios, rescaled into the ordinary normal range.
        let normal_scale = weighted_risk_adjusted_evalue(
            &losses(&[0.3, 0.7, 0.1]),
            &[-1.0, 0.5, 2.0],
            &weights(&[2.0, 5.0, 1.0]),
            1.0,
            weight(3.0),
            g,
        )
        .unwrap();
        assert_eq!(subnormal.value.as_f64(), normal_scale.value.as_f64());
    }

    #[test]
    fn subnormal_calibration_weight_stays_finite() {
        let outcome = weighted_risk_adjusted_evalue(
            &losses(&[1.0, 0.5]),
            &[0.0, 1.0],
            &weights(&[f64::MIN_POSITIVE * 0.5, 1.0]),
            0.0,
            weight(1.0),
            gamma(0.5),
        )
        .unwrap();
        assert!(outcome.value.as_f64().is_finite());
    }

    #[test]
    fn extreme_weight_ratio_stays_finite_and_non_negative() {
        let outcome = weighted_risk_adjusted_evalue(
            &losses(&[1.0, 1.0]),
            &[0.0, 1.0],
            &weights(&[1e-100, 1e100]),
            0.5,
            weight(1.0),
            gamma(0.5),
        )
        .unwrap();
        assert!(outcome.value.as_f64().is_finite() && outcome.value.as_f64() >= 0.0);
    }

    #[test]
    fn tied_score_weighted_loss_is_bit_exact_under_any_input_order() {
        // Same adversarial loss alphabet as evalue.rs's and coupled.rs's
        // analogous regression tests, this time also varying weight.
        let tied = [
            (0.1, 1.0),
            (0.2, 2.0),
            (0.3, 0.5),
            (1e-16, 3.0),
            (1e-12, 0.1),
            (1.0 - 1e-16, 4.0),
            (0.1, 2.0),
            (0.3, 1.0),
            (1e-12, 0.5),
            (0.2, 3.0),
        ];
        let score = 5.0;
        let scores = vec![score; tied.len()];
        let g = gamma(0.5);

        let forward = weighted_risk_adjusted_evalue(
            &losses(&tied.iter().map(|&(l, _)| l).collect::<Vec<_>>()),
            &scores,
            &weights(&tied.iter().map(|&(_, w)| w).collect::<Vec<_>>()),
            score,
            weight(1.0),
            g,
        )
        .unwrap();

        let mut reversed = tied.to_vec();
        reversed.reverse();
        let reversed_outcome = weighted_risk_adjusted_evalue(
            &losses(&reversed.iter().map(|&(l, _)| l).collect::<Vec<_>>()),
            &scores,
            &weights(&reversed.iter().map(|&(_, w)| w).collect::<Vec<_>>()),
            score,
            weight(1.0),
            g,
        )
        .unwrap();

        assert_eq!(forward.value.as_f64(), reversed_outcome.value.as_f64());
    }

    proptest::proptest! {
        #[test]
        fn construction_is_permutation_invariant(
            raw_triples in proptest::collection::vec((-50i32..50, 0..5usize, 1u32..20), 1..8),
            shuffle_keys in proptest::collection::vec(0i32..1000, 1..8),
            test_score_int in -50i32..50,
            test_weight_num in 1u32..20,
            gamma_num in 1u32..16,
        ) {
            let mut raw_triples = raw_triples;
            raw_triples.sort_by_key(|&(score, _, _)| score);
            raw_triples.dedup_by_key(|&mut (score, _, _)| score);
            proptest::prop_assume!(!raw_triples.iter().any(|&(score, _, _)| score == test_score_int));

            let n = raw_triples.len();
            let discrete = [0.0, 0.25, 0.5, 0.75, 1.0];
            let scores: Vec<f64> = raw_triples.iter().map(|&(s, _, _)| s as f64).collect();
            let losses: Vec<ClosedUnitInterval> = raw_triples
                .iter()
                .map(|&(_, li, _)| ClosedUnitInterval::new("loss", discrete[li]).unwrap())
                .collect();
            let calib_weights: Vec<NonNegative> = raw_triples
                .iter()
                .map(|&(_, _, w)| NonNegative::new("weight", w as f64).unwrap())
                .collect();
            let test_score = test_score_int as f64;
            let test_w = NonNegative::new("weight", test_weight_num as f64).unwrap();
            let g = gamma(gamma_num as f64 / 16.0);

            let original = weighted_risk_adjusted_evalue(
                &losses, &scores, &calib_weights, test_score, test_w, g,
            ).unwrap();

            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by_key(|&i| shuffle_keys.get(i).copied().unwrap_or(0));
            let permuted_scores: Vec<f64> = order.iter().map(|&i| scores[i]).collect();
            let permuted_losses: Vec<ClosedUnitInterval> = order.iter().map(|&i| losses[i]).collect();
            let permuted_weights: Vec<NonNegative> = order.iter().map(|&i| calib_weights[i]).collect();

            let permuted = weighted_risk_adjusted_evalue(
                &permuted_losses, &permuted_scores, &permuted_weights, test_score, test_w, g,
            ).unwrap();

            proptest::prop_assert_eq!(original.value.as_f64(), permuted.value.as_f64());
        }

        // Redundant with the type system in one sense -- a successful
        // `weighted_risk_adjusted_evalue` call can only ever construct a
        // `NonNegative` -- but fuzzed explicitly anyway (matching
        // `coupled.rs`'s analogous test) so a future refactor that
        // accidentally bypassed `NonNegative::new` would be caught by a
        // named, paper-traceable property rather than only by luck.
        #[test]
        fn evalues_are_non_negative_fuzzed(
            raw_triples in proptest::collection::vec((-50i32..50, 0..5usize, 0u32..20), 0..10),
            test_score_int in -50i32..50,
            test_weight_num in 0u32..20,
            gamma_num in 1u32..16,
        ) {
            let discrete = [0.0, 0.25, 0.5, 0.75, 1.0];
            let scores: Vec<f64> = raw_triples.iter().map(|&(s, _, _)| s as f64).collect();
            let losses: Vec<ClosedUnitInterval> = raw_triples
                .iter()
                .map(|&(_, li, _)| ClosedUnitInterval::new("loss", discrete[li]).unwrap())
                .collect();
            let calib_weights: Vec<NonNegative> = raw_triples
                .iter()
                .map(|&(_, _, w)| NonNegative::new("weight", w as f64).unwrap())
                .collect();
            proptest::prop_assume!(!losses.is_empty());
            let test_score = test_score_int as f64;
            let test_w = NonNegative::new("weight", test_weight_num as f64).unwrap();
            let g = gamma(gamma_num as f64 / 16.0);

            if let Ok(outcome) = weighted_risk_adjusted_evalue(
                &losses, &scores, &calib_weights, test_score, test_w, g,
            ) {
                proptest::prop_assert!(outcome.value.as_f64() >= 0.0);
            }
        }
    }
}
