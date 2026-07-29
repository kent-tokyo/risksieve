//! Risk-adjusted e-values (Definition 3.1) and their construction from a
//! calibration set and a score function (Equation 4.1).
//!
//! Bai and Jin (2026), *Conformal Selective Prediction with General Risk
//! Control*, arXiv:2603.24704, Definition 3.1: for the random risk
//! `L_{n+1} = loss(f, X_{n+1}, Y_{n+1})`, a random variable `E_{n+1}` is a
//! *risk-adjusted e-value* if `E_{n+1} >= 0` almost surely and
//! `E[E_{n+1} * L_{n+1}] <= 1`. The loss is normalized to `[0, 1]`
//! ("without loss of generality" per the paper's problem setup); this is
//! why calibration losses here are [`ClosedUnitInterval`], not the more
//! general [`crate::probability::ClosedInterval`] every other controller
//! in this crate uses.
//!
//! [`risk_adjusted_evalue`] implements the concrete construction from
//! Equation (4.1):
//!
//! ```text
//! E_{gamma,n+1} = inf_{l in [0,1]} (n+1) * 1{s(X_{n+1}) <= t_gamma(l)}
//!                 / ( sum_{i=1}^n L_i * 1{s(X_i) <= t_gamma(l)}
//!                     + l * 1{s(X_{n+1}) <= t_gamma(l)} )
//!
//! t_gamma(l) = max{ t in M : F(t;l) <= gamma }
//! F(t;l) = ( sum_{i=1}^n L_i * 1{s(X_i) <= t} + l * 1{s(X_{n+1}) <= t} ) / (n+1)
//! ```
//!
//! where `M = {s(X_1), ..., s(X_n), s(X_{n+1})}`. Theorem 4.2: for any
//! fixed `gamma in (0,1)` and exchangeable `{(X_i,Y_i)}_{i=1}^{n+1}`,
//! `E[L_{n+1} * E_{gamma,n+1}] <= 1` -- validity holds for *any* `gamma`,
//! not only `gamma <= alpha`; Remark 4.5 is about statistical power, not
//! validity (see [`crate::selective::mdr`]).
//!
//! **The score function `s(.)` need not be calibrated or accurate.**
//! Definition 3.1's validity does not depend on `s` predicting risk well
//! -- it only needs to be a fixed, pre-trained function evaluated on
//! calibration and test covariates. A poor score can still severely
//! reduce *selection power* (how often a genuinely trustworthy point gets
//! deployed), even though it never breaks the guarantee itself.
//!
//! ## How the infimum is computed
//!
//! `F(t;l)` is non-decreasing in both `t` and `l` (each term is an
//! indicator times a non-negative quantity, and indicators only turn on,
//! never off, as `t` grows), so the set of thresholds satisfying
//! `F(t;l) <= gamma` is a downward-closed prefix of the sorted distinct
//! values in `M`, and `t_gamma(l)` -- the largest such threshold -- is
//! non-increasing in `l`. Within any range of `l` where `t_gamma(l)` is
//! constant, Equation (4.1)'s objective is either identically `0` (the
//! test point's own score exceeds that threshold, so its indicator is
//! `0`) or strictly decreasing in `l` (the test point's score is at or
//! below the threshold, so larger `l` only grows the denominator). Either
//! way the minimum within that range sits at its upper (larger-`l`) end.
//! This means the global infimum over `l in [0,1]` is attained at one of
//! finitely many breakpoints -- `0`, `1`, and the `l` at which each
//! eligible threshold's constraint turns from satisfied to violated --
//! rather than requiring a numerical search. `tests/paper_score_mdr.rs`
//! has the derivation worked through on two small hand-computed examples.
//!
//! This reference implementation evaluates the objective directly at
//! every breakpoint (`O(n log n)` for the sort, `O(n)` breakpoints each
//! re-scanned in `O(n)`, so `O(n^2)` overall) rather than tracking
//! plateaus incrementally. Proposition 4.4 gives a closed-form shortcut
//! for the special case `gamma <= alpha` that avoids this scan entirely;
//! it is verified against this reference by a property test but not
//! wired in as a separate code path (see `docs/roadmap.md` -- this is not a
//! hot loop, so the simpler `O(n^2)` reference is used unconditionally
//! until profiling says otherwise).
//!
//! **Provenance:** this paper postdates this project's training-data
//! cutoff. Definition 3.1, Equation 4.1/4.2, Theorem 4.2, and Remark 4.5
//! were cross-checked across four independent fetches of the paper's own
//! HTML rendering (arxiv.org and ar5iv, two separate revisions) that
//! agreed on every formula and constant; the `[0,1]` loss normalization
//! and the exchangeability requirement over `{(X_i,Y_i)}_{i=1}^{n+1}`
//! were separately confirmed from the paper's problem-setup section.

use crate::error::RiskSieveError;
use crate::probability::{ClosedUnitInterval, NonNegative, OpenUnitInterval, check_finite};

/// The result of evaluating [Definition 3.1] via [Equation 4.1].
///
/// [Definition 3.1]: Definition 3.1 (this module's docs)
/// [Equation 4.1]: Equation 4.1 (this module's docs)
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EValueOutcome {
    /// The computed risk-adjusted e-value `E_{gamma,n+1}`.
    pub value: NonNegative,
    /// `false` when the value minimizing Equation 4.1 comes from the case
    /// where *no* threshold in `M` satisfies `F(t;l) <= gamma` for any
    /// `l` -- a degenerate input (calibration set too small, or `gamma`
    /// too strict for it), not a genuinely informative small-risk result.
    /// [`crate::selective::mdr::certify`] surfaces this via
    /// `Diagnostics::uninformative_result`.
    pub feasible_threshold_found: bool,
}

/// A few ULPs of slack around `gamma_scaled`, scaled to its magnitude.
///
/// Candidate `l` values are constructed as `gamma_scaled - base_sum[j]`;
/// adding `base_sum[j]` back should reproduce `gamma_scaled` to within a
/// rounding error or two, not a fixed absolute tolerance (which would be
/// a no-op at large `n` and needlessly generous at small `n`). The slack
/// can only ever admit a slightly *larger* threshold `t`, which grows the
/// denominator and so only ever makes the computed e-value slightly
/// *smaller* -- it rounds toward abstention, never toward a wrongly
/// deployed decision (AGENTS.md section 8).
fn feasibility_epsilon(gamma_scaled: f64) -> f64 {
    gamma_scaled.abs().max(1.0) * 8.0 * f64::EPSILON
}

/// Computes the risk-adjusted e-value `E_{gamma,n+1}` (Equation 4.1) for
/// one test point against a calibration set.
///
/// `calibration_losses[i]` and `calibration_scores[i]` must correspond to
/// the same calibration point `i`; both slices must have equal, nonzero
/// length. `calibration_scores` and `test_score` are the pre-trained
/// score function `s(.)` evaluated at each covariate -- any finite `f64`
/// ordering is accepted, since `s` need not be calibrated (see the module
/// docs).
///
/// # Errors
///
/// - [`RiskSieveError::AssumptionMismatch`] if the two slices have
///   different lengths.
/// - [`RiskSieveError::EmptyCalibrationSet`] if they are empty.
/// - [`RiskSieveError::NonFiniteValue`] if `test_score` or any
///   calibration score is NaN or infinite.
///
/// # Example
///
/// ```
/// use risksieve::selective::evalue::risk_adjusted_evalue;
/// use risksieve::{ClosedUnitInterval, OpenUnitInterval};
///
/// let losses = [ClosedUnitInterval::new("loss", 1.0)?];
/// let scores = [0.0];
/// let outcome = risk_adjusted_evalue(&losses, &scores, 1.0, OpenUnitInterval::new("gamma", 0.5)?)?;
/// // Worked out in tests/paper_score_mdr.rs: the test point's score (1.0)
/// // exceeds the only calibration score (0.0) for every `l > 0`, so the
/// // objective is 0 almost everywhere on `[0,1]` and the infimum is 0.
/// assert_eq!(outcome.value.get(), 0.0);
/// # Ok::<(), risksieve::RiskSieveError>(())
/// ```
pub fn risk_adjusted_evalue(
    calibration_losses: &[ClosedUnitInterval],
    calibration_scores: &[f64],
    test_score: f64,
    gamma: OpenUnitInterval,
) -> Result<EValueOutcome, RiskSieveError> {
    if calibration_losses.len() != calibration_scores.len() {
        return Err(RiskSieveError::AssumptionMismatch {
            detail: format!(
                "calibration_losses has {} entries but calibration_scores has {}",
                calibration_losses.len(),
                calibration_scores.len(),
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

    // `-0.0 == 0.0` but `f64::total_cmp` orders them as distinct, which
    // would silently split a tie at exactly zero into two threshold
    // candidates.
    let normalize_zero = |x: f64| if x == 0.0 { 0.0 } else { x };
    let test_score = normalize_zero(test_score);

    let n = calibration_losses.len();
    let n_plus_1 = (n + 1) as f64;
    let gamma_scaled = gamma.get() * n_plus_1;
    let epsilon = feasibility_epsilon(gamma_scaled);

    // Group into distinct sorted score values with their per-value loss
    // sum -- a zero-loss placeholder represents the test point's own
    // score, since `M` includes it alongside the calibration scores.
    let mut entries: Vec<(f64, f64)> = calibration_scores
        .iter()
        .zip(calibration_losses.iter())
        .map(|(&score, &loss)| (normalize_zero(score), loss.get()))
        .collect();
    entries.push((test_score, 0.0));
    entries.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut values: Vec<f64> = Vec::new();
    let mut per_value_sum: Vec<f64> = Vec::new();
    for (score, loss) in entries {
        if values.last() == Some(&score) {
            *per_value_sum
                .last_mut()
                .expect("values and per_value_sum stay in lockstep") += loss;
        } else {
            values.push(score);
            per_value_sum.push(loss);
        }
    }

    // Compensated running prefix sum (AGENTS.md section 8): base_sum[j]
    // is the total calibration loss at scores <= values[j].
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

    let largest_feasible_index = |ell: f64| -> Option<usize> {
        (0..base_sum.len()).rev().find(|&j| {
            let contribution = base_sum[j] + if test_below[j] { ell } else { 0.0 };
            contribution <= gamma_scaled + epsilon
        })
    };

    let objective_at = |ell: f64, j: Option<usize>| -> f64 {
        match j {
            Some(j) if test_below[j] => n_plus_1 / (base_sum[j] + ell),
            _ => 0.0,
        }
    };

    // Breakpoint candidates: see the module docs for why the infimum is
    // attained at one of these.
    let mut candidates: Vec<f64> = vec![0.0, 1.0];
    for (j, &below) in test_below.iter().enumerate() {
        if below {
            candidates.push((gamma_scaled - base_sum[j]).clamp(0.0, 1.0));
        }
    }

    let mut best_value = f64::INFINITY;
    let mut best_feasible = true;
    for &ell in &candidates {
        let j = largest_feasible_index(ell);
        let value = objective_at(ell, j);
        if value < best_value {
            best_value = value;
            best_feasible = j.is_some();
        }
    }

    Ok(EValueOutcome {
        value: NonNegative::new("risk_adjusted_evalue", best_value)?,
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

    #[test]
    fn rejects_mismatched_lengths() {
        let err = risk_adjusted_evalue(
            &losses(&[1.0]),
            &[0.0, 1.0],
            0.5,
            OpenUnitInterval::new("gamma", 0.5).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn rejects_empty_calibration() {
        let err = risk_adjusted_evalue(&[], &[], 0.5, OpenUnitInterval::new("gamma", 0.5).unwrap())
            .unwrap_err();
        assert!(matches!(err, RiskSieveError::EmptyCalibrationSet));
    }

    #[test]
    fn rejects_non_finite_test_score() {
        let err = risk_adjusted_evalue(
            &losses(&[1.0]),
            &[0.0],
            f64::NAN,
            OpenUnitInterval::new("gamma", 0.5).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(err, RiskSieveError::NonFiniteValue { .. }));
    }

    /// Hand trace (n=1, s_1=0.0, L_1=1.0, s*=1.0, gamma=0.5, n+1=2):
    /// t_gamma(0) = 1.0 (both t=0.0 and t=1.0 satisfy F<=1.0 at l=0, max
    /// picks the larger), giving objective 2*1/(1.0+0)=2.0; for any
    /// l > 0, t=1.0 requires F=1.0+l<=1.0 which fails, so t_gamma(l)=0.0
    /// and the test point's indicator is 0, giving objective 0. The
    /// infimum over [0,1] is therefore 0. See also the doc-test above.
    #[test]
    fn matches_hand_computation_test_point_excluded() {
        let outcome = risk_adjusted_evalue(
            &losses(&[1.0]),
            &[0.0],
            1.0,
            OpenUnitInterval::new("gamma", 0.5).unwrap(),
        )
        .unwrap();
        assert_eq!(outcome.value.get(), 0.0);
        assert!(outcome.feasible_threshold_found);
    }

    /// Hand trace (n=1, s_1=1.0, L_1=0.0, s*=0.0, gamma=0.5, n+1=2): both
    /// M-values give base_sum=0, and the test point's indicator is 1 at
    /// both thresholds (its own score is the smallest). At l=0 the
    /// denominator is exactly 0 (objective = 2/0 = +infinity); at l=1 the
    /// denominator is 1 (objective = 2.0). The infimum is 2.0.
    #[test]
    fn matches_hand_computation_zero_denominator_candidate() {
        let outcome = risk_adjusted_evalue(
            &losses(&[0.0]),
            &[1.0],
            0.0,
            OpenUnitInterval::new("gamma", 0.5).unwrap(),
        )
        .unwrap();
        assert_eq!(outcome.value.get(), 2.0);
        assert!(outcome.feasible_threshold_found);
    }

    #[test]
    fn no_feasible_threshold_is_flagged_uninformative() {
        // A single calibration point with loss 1.0 at the same score as
        // the test point, and gamma so strict that even t = the smallest
        // M-value fails F(t;l) <= gamma for every l (gamma*(n+1) < 1.0
        // and the calibration point's own loss alone already exceeds
        // it, regardless of the test point's l-dependent contribution).
        let outcome = risk_adjusted_evalue(
            &losses(&[1.0]),
            &[0.0],
            0.0,
            OpenUnitInterval::new("gamma", 0.1).unwrap(),
        )
        .unwrap();
        assert_eq!(outcome.value.get(), 0.0);
        assert!(!outcome.feasible_threshold_found);
    }

    #[test]
    fn ties_at_the_same_score_are_summed() {
        // Two calibration points tied at the same score as the test
        // point; both losses must land in the same `M` bucket.
        let outcome = risk_adjusted_evalue(
            &losses(&[0.5, 0.5]),
            &[2.0, 2.0],
            2.0,
            OpenUnitInterval::new("gamma", 0.9).unwrap(),
        )
        .unwrap();
        // n=2, n+1=3, gamma*(n+1)=2.7; base_sum at the shared value is
        // 1.0 (0.5+0.5); at l=1, contribution=1.0+1=2.0<=2.7, feasible,
        // objective=3/(1.0+1.0)=1.5; l=0 gives 3/(1.0+0)=3.0. Min is 1.5.
        assert!((outcome.value.get() - 1.5).abs() < 1e-12);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn evalue_outcome_serde_round_trip() {
        let outcome = EValueOutcome {
            value: NonNegative::new("risk_adjusted_evalue", 2.0).unwrap(),
            feasible_threshold_found: true,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let restored: EValueOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, outcome);
    }

    // AGENTS.md section 9.3: "permutation invariance of symmetric
    // procedures." Equation 4.1's construction only depends on the
    // calibration set as a multiset of (score, loss) pairs -- never on
    // input order -- which is exactly why exchangeability suffices for
    // Theorem 4.2's validity claim. Distinct scores are used so there is
    // no tie-breaking ambiguity for the permutation to expose; ties are
    // covered separately by `ties_at_the_same_score_are_summed` above.
    proptest::proptest! {
        #[test]
        fn construction_is_permutation_invariant(
            raw_pairs in proptest::collection::vec((-50i32..50, 0..5usize), 1..8),
            shuffle_keys in proptest::collection::vec(0i32..1000, 1..8),
            test_score_int in -50i32..50,
            gamma_num in 1u32..16,
        ) {
            let mut raw_pairs = raw_pairs;
            raw_pairs.sort_by_key(|&(score, _)| score);
            raw_pairs.dedup_by_key(|&mut (score, _)| score);
            proptest::prop_assume!(!raw_pairs.iter().any(|&(score, _)| score == test_score_int));

            let n = raw_pairs.len();
            let discrete = [0.0, 0.25, 0.5, 0.75, 1.0];
            let scores: Vec<f64> = raw_pairs.iter().map(|&(s, _)| s as f64).collect();
            let losses: Vec<ClosedUnitInterval> = raw_pairs
                .iter()
                .map(|&(_, li)| ClosedUnitInterval::new("loss", discrete[li]).unwrap())
                .collect();
            let test_score = test_score_int as f64;
            let gamma = OpenUnitInterval::new("gamma", gamma_num as f64 / 16.0).unwrap();

            let original = risk_adjusted_evalue(&losses, &scores, test_score, gamma).unwrap();

            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by_key(|&i| shuffle_keys.get(i).copied().unwrap_or(0));
            let permuted_scores: Vec<f64> = order.iter().map(|&i| scores[i]).collect();
            let permuted_losses: Vec<ClosedUnitInterval> = order.iter().map(|&i| losses[i]).collect();

            let permuted =
                risk_adjusted_evalue(&permuted_losses, &permuted_scores, test_score, gamma).unwrap();

            proptest::prop_assert_eq!(original.value.get(), permuted.value.get());
        }
    }
}
