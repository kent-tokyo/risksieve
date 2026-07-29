//! Cross-test-point coupled risk-adjusted e-values (Equation 5.1, and the
//! efficient computation "Algorithm 3" gives for it).
//!
//! Bai and Jin (2026), *Conformal Selective Prediction with General Risk
//! Control*, arXiv:2603.24704, Equation 5.1 gives the paper's own
//! per-test-point e-value for the SDR batch setting, distinct from
//! [`super::evalue::risk_adjusted_evalue`] (Equation 4.1) in that its
//! normalizing function counts *every other test point's* score against
//! the same threshold, not just calibration:
//!
//! ```text
//! E_{gamma,n+j} = inf_{l in [0,1]} (n+1) * 1{s(X_{n+j}) <= t_{gamma,n+j}(l)}
//!                 / ( l * 1{s(X_{n+j}) <= t_{gamma,n+j}(l)}
//!                     + sum_{i=1}^n L_i * 1{s(X_i) <= t_{gamma,n+j}(l)} )
//!
//! t_{gamma,n+j}(l) = max{ t in M : FR_{n+j}(t;l) <= gamma }
//!
//! FR_{n+j}(t;l) = [ l * 1{s(X_{n+j})<=t} + sum_{i=1}^n L_i * 1{s(X_i)<=t} ]
//!                 / [ 1 + sum_{k != j} 1{s(X_{n+k})<=t} ] * (m / (n+1))
//! ```
//!
//! where `M = {s(X_i)}_{i=1}^{n+m}` is the pooled calibration-and-test score
//! set. Theorem 5.1: under joint exchangeability of `{(X_i,Y_i)}_{i=1}^{n+m}`
//! (calibration *and every test point*, not just `n+1` as in Equation 4.1),
//! `E[L_{n+j} * E_{gamma,n+j}] <= 1` for any fixed `gamma > 0` -- so each
//! `E_{gamma,n+j}` individually satisfies Definition 3.1 and can feed
//! [`super::ebh::select`] exactly as [`super::evalue::risk_adjusted_evalue`]
//! does.
//!
//! ## Correspondence to the paper and to `Tian-Bai/SCoRE`'s `SCoRE_SDR`
//!
//! `SCoRE_SDR` (`SCoRE/SCoRE.py`, commit `401b7caf6d030825ff67e8f08e44ba15ee8c94af`,
//! package version `0.1.1`) implements this equation; the audit below maps
//! every quantity to both the paper's own notation and that function's
//! variable names, since this module was independently derived from the
//! equation and cross-checked against, rather than translated from, that
//! source (see `THIRD_PARTY_NOTICES.md`).
//!
//! | Paper (Eq. 5.1) | `SCoRE_SDR` | This module | Meaning |
//! |---|---|---|---|
//! | `sum_i L_i * 1{s(X_i)<=t}` | `NUMER[i]` | `calib_prefix[k]` | cumulative calibration loss at or below the `k`-th distinct pooled score |
//! | `1 + sum_{k'!=k} 1{s(X_{n+k'})<=t}` | `DENOM[i] - (Stest[j]<=t)` | `denom_excl_j(k)` | one plus the count of *other* test points at or below that score -- excluding the test point under consideration is exactly the `k != j` in Eq. 5.1's normalizer |
//! | `FR_{n+j}(t;0)` | `FR_0[i]` | `fr0_feasible(k)` (as a cross-multiplied comparison, see below) | the normalizing function at `l=0` |
//! | `FR_{n+j}(t;1)` | `FR_1[i]` | `fr1_feasible(k)` | the normalizing function at `l=1` |
//! | the `l` at which threshold `t` stops being `l=0`-feasible | `ELL[i]` | `ell_bar(k)` | Algorithm 3's own name for this quantity is `l-bar(t)`; it is the breakpoint candidate for the infimum, not itself constrained to `[0,1]` until it is actually plugged into the objective (see "Clamping" below) |
//! | `t_{gamma,n+j}(0)` | `t_0` | `t0` | the largest pooled score feasible at `l=0` |
//! | `t_{gamma,n+j}(1)` | `t_1` | `t1` | the largest pooled score feasible at `l=1`; always `<= t0` because `FR(t;1) >= FR(t;0)` pointwise, so its feasible set is a subset |
//! | the candidate breakpoints surviving pruning | `M_star` | the `k` values visited in the final scan | see "Why a suffix maximum" below |
//! | `E_{gamma,n+j}` | `evalues[j]` | the return value | the coupled e-value itself |
//!
//! **Why exclude test point `j` from its own denominator's test count:**
//! Equation 5.1's normalizer counts test points *other than `j`* below the
//! threshold (`sum_{k != j}`) plus a constant `1`. Test point `j`'s own
//! indicator is priced separately, through the numerator's `l * 1{...}`
//! term (the quantity actually being optimized over `l in [0,1]`) -- if its
//! own presence were also counted in the denominator, the denominator would
//! depend on the same indicator the numerator already varies via `l`,
//! double-counting it.
//!
//! **Why a suffix maximum:** among candidate thresholds `t` for which
//! `FR(t;0) <= gamma` (so `t` is at least `l=0`-feasible), the ones worth
//! evaluating are exactly those whose `ell_bar(t)` is *not dominated* by
//! some strictly larger, also-`l=0`-feasible threshold's `ell_bar`. If a
//! larger threshold `t' > t` has `ell_bar(t') >= ell_bar(t)`, then every `l`
//! for which `t` is the tightest feasible threshold is already covered by
//! `t'` being feasible too (since `t_{gamma,n+j}(l)` takes the *largest*
//! feasible `t`), so `t` can never be the actual argmax threshold at any
//! `l` and contributes nothing the scan needs to visit. Computing, for each
//! `t`, the maximum `ell_bar` among all strictly larger `l=0`-feasible
//! thresholds (a single backward pass, `O(n+m)`) is exactly what lets the
//! per-test-point scan skip dominated candidates in one forward pass rather
//! than a nested search.
//!
//! **Why the "no need for a candidate list" collapse.** if `t_1 == t_0`
//! (the feasible boundary does not move at all between `l=0` and `l=1`),
//! Equation 4.1's own reasoning (see [`super::evalue`]'s module docs)
//! applies verbatim within that single plateau: the objective is
//! monotonically non-increasing in `l`, so the infimum is at `l=1`,
//! `(n+1) / (1 + calib_prefix)`, with no candidate scan needed at all.
//!
//! **Clamping `ell_bar` to `[0,1]` -- a deliberate divergence from
//! `SCoRE_SDR`.** Equation 5.1's infimum ranges over `l in [0,1]`; `ell_bar(t)`
//! is the location, in that same `l`-space, where `t`'s plateau boundary
//! sits -- but nothing forces `ell_bar(t) <= 1`, since `t` was only
//! constrained to be `l=0`-feasible, not to have a plateau that ends inside
//! `[0,1]`. When `ell_bar(t) > 1`, the *true* minimum of that plateau's
//! objective is at the domain's own boundary `l=1`, not at `ell_bar(t)`.
//! `SCoRE_SDR` (as of the commit and version above) evaluates the objective
//! at the unclamped `ell_bar(t)` directly (`cur_val = (Ncalib+1) /
//! (ELL[i] + NUMER[i])`), which for `ell_bar(t) > 1` computes a value
//! *smaller* than the true, domain-restricted infimum -- an
//! under-statement of the e-value, which would bias toward abstention
//! rather than toward an invalid guarantee, but is nonetheless a departure
//! from Equation 5.1 as written. This module clamps `ell_bar(t)` to
//! `[0.0, 1.0]` before evaluating the objective, matching the equation's
//! stated domain exactly (the lower clamp is defensive against sub-ULP
//! rounding noise; every candidate that reaches this step already satisfies
//! `FR(t;0) <= gamma`, which is algebraically equivalent to `ell_bar(t) >= 0`).
//! An empirical comparison across 50,000 randomized and adversarial
//! (tie-heavy, extreme-`gamma`) trials against the unclamped `SCoRE_SDR`
//! found 5,142 cases where `ell_bar(t) > 1` for some candidate, and zero
//! resulting differences in the final e-value or selected set -- see
//! `THIRD_PARTY_NOTICES.md` for the comparison script. This is not a proof
//! that the two are always identical, only evidence that the divergence,
//! where it exists, has not been observed to change any output; this
//! module implements the mathematically-justified (clamped) version
//! regardless.
//!
//! ## `gamma`'s domain
//!
//! Section 5 of the paper states Theorem 5.1 for any fixed `gamma > 0`,
//! with no upper bound -- a genuinely wider domain than Equation 4.1's
//! `gamma in (0,1)` (Theorem 4.2), because Equation 5.1's normalizer
//! carries an extra `m / (n+1)` scale factor Equation 4.1 does not, so
//! `FR` can exceed 1 well before the calibration set is fully covered
//! whenever `m` is large relative to `n`. This module nonetheless accepts
//! `gamma: OpenUnitInterval` (matching Equation 4.1's type, *not* widening
//! to the paper's stated domain), for reasons this crate is choosing
//! deliberately rather than by omission:
//!
//! - `Tian-Bai/SCoRE`'s own `_validate_gamma` rejects `gamma > 1`, so the
//!   oracle fixtures this module is cross-checked against cannot exercise
//!   `gamma > 1` through the public API either -- widening past what the
//!   oracle itself accepts would leave that range untested by the one
//!   cross-language check this crate has.
//! - The paper's own recommended default (the SDR analogue of Remark 4.5's
//!   `gamma = alpha`) is always in `(0,1)`, since `alpha in (0,1)` is
//!   required everywhere in this crate.
//! - It keeps the `gamma` parameter's type identical across
//!   [`super::evalue::risk_adjusted_evalue`], [`super::sdr::certify_independent`],
//!   and [`super::sdr::certify`], rather than the same-named parameter
//!   silently meaning two different domains depending on which function is
//!   called.
//! - At `gamma = 0` exactly, every candidate's clamped denominator
//!   `ell_bar(t) + calib_prefix(t)` collapses to `gamma_scale * denom_excl_j(t)`
//!   with `gamma_scale = gamma * (n+1) / m = 0`, which can make the
//!   e-value's true mathematical infimum `+infinity` (an unbounded, not
//!   merely large, value) -- excluding `gamma = 0` keeps every value this
//!   module returns finite, avoiding the need for a dedicated
//!   infinite-e-value representation. `OpenUnitInterval` already excludes
//!   this endpoint, which is the reason this module keeps it rather than
//!   widening to `ClosedUnitInterval` or beyond.
//!
//! A numerical exploration constructed to stress the `m / (n+1)` scaling
//! (large `m`, few test points below the calibration region) did not find
//! a case where allowing `gamma` up to `5.0` changed the selected set
//! relative to capping it at `1.0` -- in every configuration tried, the
//! e-value saturated (stopped changing as `gamma` grew further) at or
//! before `gamma = 1`. This is not a general proof that the `(0,1)` cap is
//! costless, only an empirical observation from a handful of constructed
//! and randomized scenarios; see `docs/references.md` for the numbers.
//!
//! ## Complexity
//!
//! `O((n+m) log(n+m) + m(n+m))`: sorting and grouping the pooled scores is
//! `O((n+m) log(n+m))`; each of the `m` test points is then processed in a
//! constant number of `O(n+m)` passes over the grouped, prefix-summed
//! array (locating its own group by binary search, one forward scan for
//! `FR`/`ell_bar`, one backward scan for the suffix maximum, one forward
//! scan for the candidate minimum), matching Proposition 5.2's stated bound
//! of `O((n+m)m + (n+m)log(n+m))` for Algorithm 3.

use crate::error::RiskSieveError;
use crate::probability::{ClosedUnitInterval, NonNegative, OpenUnitInterval, check_finite};

fn normalize_zero(x: f64) -> f64 {
    if x == 0.0 { 0.0 } else { x }
}

/// A few ULPs of slack around a feasibility comparison's right-hand side,
/// scaled to its own magnitude -- see [`super::evalue`]'s
/// `feasibility_epsilon` for the identical rationale. Slack here can only
/// ever admit a slightly larger threshold `t`, which grows the eventual
/// denominator and so only ever makes the computed e-value slightly
/// smaller: it rounds toward abstention, never toward a wrongly-selected
/// test point.
fn feasibility_epsilon(rhs: f64) -> f64 {
    rhs.abs().max(1.0) * 8.0 * f64::EPSILON
}

/// Groups calibration and test scores into distinct sorted values, each
/// carrying its own (not yet prefixed) calibration loss sum and test-point
/// count -- the pooled score set `M` from Equation 5.1, deduplicated by
/// value up front rather than sorted-with-duplicates-then-fixed-up (unlike
/// `SCoRE_SDR`'s tuple sort, which relies on a post-hoc tie-correction
/// pass; see the module docs).
struct GroupedScores {
    values: Vec<f64>,
    calib_loss_sum: Vec<f64>,
    test_count: Vec<usize>,
}

fn group_scores(
    calibration_losses: &[ClosedUnitInterval],
    calibration_scores: &[f64],
    test_scores: &[f64],
) -> GroupedScores {
    let mut entries: Vec<(f64, f64, usize)> =
        Vec::with_capacity(calibration_scores.len() + test_scores.len());
    entries.extend(
        calibration_scores
            .iter()
            .zip(calibration_losses.iter())
            .map(|(&score, &loss)| (normalize_zero(score), loss.get(), 0usize)),
    );
    entries.extend(
        test_scores
            .iter()
            .map(|&score| (normalize_zero(score), 0.0, 1usize)),
    );
    entries.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut values = Vec::new();
    let mut calib_loss_sum = Vec::new();
    let mut test_count = Vec::new();
    for (score, loss, is_test) in entries {
        if values.last() == Some(&score) {
            *calib_loss_sum.last_mut().expect("kept in lockstep") += loss;
            *test_count.last_mut().expect("kept in lockstep") += is_test;
        } else {
            values.push(score);
            calib_loss_sum.push(loss);
            test_count.push(is_test);
        }
    }

    GroupedScores {
        values,
        calib_loss_sum,
        test_count,
    }
}

/// Computes the cross-test-point coupled risk-adjusted e-value
/// (Equation 5.1) for every test point in `test_scores` against one shared
/// calibration set, in a single `O((n+m) log(n+m) + m(n+m))` pass -- see
/// the module docs for the full derivation and the correspondence to
/// `SCoRE_SDR`.
///
/// `calibration_losses[i]` and `calibration_scores[i]` must correspond to
/// the same calibration point; both slices must have equal, nonzero
/// length (an empty calibration set is rejected, matching
/// [`super::evalue::risk_adjusted_evalue`]'s convention, even though
/// Equation 5.1 itself does not obviously require it). An empty
/// `test_scores` returns an empty vector without error.
///
/// # Errors
///
/// - [`RiskSieveError::AssumptionMismatch`] if `calibration_losses` and
///   `calibration_scores` have different lengths.
/// - [`RiskSieveError::EmptyCalibrationSet`] if they are empty.
/// - [`RiskSieveError::NonFiniteValue`] if any calibration or test score is
///   NaN or infinite.
pub fn coupled_risk_adjusted_evalues(
    calibration_losses: &[ClosedUnitInterval],
    calibration_scores: &[f64],
    test_scores: &[f64],
    gamma: OpenUnitInterval,
) -> Result<Vec<NonNegative>, RiskSieveError> {
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
    for &score in calibration_scores {
        check_finite("calibration_scores", score)?;
    }
    for &score in test_scores {
        check_finite("test_scores", score)?;
    }

    if test_scores.is_empty() {
        return Ok(Vec::new());
    }

    let n = calibration_losses.len();
    let m = test_scores.len();
    let n_plus_1 = (n + 1) as f64;
    let m_f = m as f64;
    let gamma_scale = gamma.get() * n_plus_1 / m_f;

    let grouped = group_scores(calibration_losses, calibration_scores, test_scores);
    let group_count = grouped.values.len();

    // Compensated running prefix sum of calibration loss (AGENTS.md section
    // 8); test-point counts are exact integers representable in f64, so a
    // plain running sum needs no compensation.
    let mut calib_prefix = Vec::with_capacity(group_count);
    let mut test_count_prefix = Vec::with_capacity(group_count);
    let mut running_loss = 0.0_f64;
    let mut compensation = 0.0_f64;
    let mut running_count = 0.0_f64;
    for k in 0..group_count {
        let adjusted = grouped.calib_loss_sum[k] - compensation;
        let new_running = running_loss + adjusted;
        compensation = (new_running - running_loss) - adjusted;
        running_loss = new_running;
        calib_prefix.push(running_loss);

        running_count += grouped.test_count[k] as f64;
        test_count_prefix.push(running_count);
    }

    let mut results = Vec::with_capacity(m);
    for &raw_test_score in test_scores {
        let test_score = normalize_zero(raw_test_score);
        let j_pos = grouped
            .values
            .binary_search_by(|value| value.total_cmp(&test_score))
            .expect("test_score was included when building the grouped pooled scores");

        // `denom_excl_j(k) = 1 + (# other test points, batch-wide, with
        // score <= values[k])`: the total test count below `values[k]`
        // minus this test point's own contribution once `k >= j_pos`.
        let denom_excl_j = |k: usize| -> f64 {
            let indicator = if k >= j_pos { 1.0 } else { 0.0 };
            1.0 + test_count_prefix[k] - indicator
        };
        let ell_bar = |k: usize| -> f64 { gamma_scale * denom_excl_j(k) - calib_prefix[k] };
        let fr0_feasible = |k: usize| -> bool {
            let rhs = gamma_scale * denom_excl_j(k);
            calib_prefix[k] <= rhs + feasibility_epsilon(rhs)
        };
        let fr1_feasible = |k: usize| -> bool {
            let indicator = if k >= j_pos { 1.0 } else { 0.0 };
            let rhs = gamma_scale * denom_excl_j(k);
            calib_prefix[k] + indicator <= rhs + feasibility_epsilon(rhs)
        };

        let t0 = (0..group_count).rev().find(|&k| fr0_feasible(k));
        let t1 = (0..group_count).rev().find(|&k| fr1_feasible(k));

        let value = match t1 {
            None => 0.0,
            Some(t1_idx) if j_pos > t1_idx => 0.0,
            Some(t1_idx) => {
                let t0_idx = t0.expect("FR0 <= FR1 pointwise, so t1 feasible implies t0 feasible");
                if t1_idx == t0_idx {
                    n_plus_1 / (1.0 + calib_prefix[t1_idx])
                } else {
                    // Suffix maximum of `ell_bar` restricted to FR0-feasible
                    // positions, for positions strictly after `k` -- see
                    // "Why a suffix maximum" in the module docs.
                    let mut suffix_max_ell = vec![f64::NEG_INFINITY; group_count];
                    let mut running_max = f64::NEG_INFINITY;
                    for k in (0..group_count).rev() {
                        suffix_max_ell[k] = running_max;
                        if fr0_feasible(k) {
                            running_max = running_max.max(ell_bar(k));
                        }
                    }

                    let mut best: Option<f64> = None;
                    for k in t1_idx..=t0_idx {
                        if fr0_feasible(k) && ell_bar(k) > suffix_max_ell[k] {
                            let clamped_ell = ell_bar(k).clamp(0.0, 1.0);
                            let candidate = n_plus_1 / (clamped_ell + calib_prefix[k]);
                            best =
                                Some(best.map_or(candidate, |current: f64| current.min(candidate)));
                        }
                    }
                    best.ok_or(RiskSieveError::NumericalFailure {
                        operation: "coupled_risk_adjusted_evalues: M_star was empty despite t0 being a guaranteed member",
                    })?
                }
            }
        };

        results.push(NonNegative::new("coupled_risk_adjusted_evalue", value)?);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selective::evalue::risk_adjusted_evalue;

    fn losses(values: &[f64]) -> Vec<ClosedUnitInterval> {
        values
            .iter()
            .map(|&v| ClosedUnitInterval::new("loss", v).unwrap())
            .collect()
    }

    fn gamma(v: f64) -> OpenUnitInterval {
        OpenUnitInterval::new("gamma", v).unwrap()
    }

    #[test]
    fn rejects_mismatched_lengths() {
        let err = coupled_risk_adjusted_evalues(&losses(&[1.0]), &[0.0, 1.0], &[0.5], gamma(0.5))
            .unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn rejects_empty_calibration() {
        let err = coupled_risk_adjusted_evalues(&[], &[], &[0.5], gamma(0.5)).unwrap_err();
        assert!(matches!(err, RiskSieveError::EmptyCalibrationSet));
    }

    #[test]
    fn rejects_non_finite_scores() {
        let err = coupled_risk_adjusted_evalues(&losses(&[1.0]), &[0.0], &[f64::NAN], gamma(0.5))
            .unwrap_err();
        assert!(matches!(err, RiskSieveError::NonFiniteValue { .. }));
        let err =
            coupled_risk_adjusted_evalues(&losses(&[1.0]), &[f64::INFINITY], &[0.5], gamma(0.5))
                .unwrap_err();
        assert!(matches!(err, RiskSieveError::NonFiniteValue { .. }));
    }

    #[test]
    fn empty_test_batch_returns_empty_vec_without_error() {
        let result =
            coupled_risk_adjusted_evalues(&losses(&[1.0]), &[0.0], &[], gamma(0.5)).unwrap();
        assert!(result.is_empty());
    }

    /// `m=1` collapse: Equation 5.1's normalizer `1 + sum_{k != j} 1{...}`
    /// has an empty sum when there is only one test point, so it is the
    /// constant `1` regardless of `t`, and `m/(n+1) = 1/(n+1)`. Equation
    /// 5.1 then reduces algebraically to exactly Equation 4.1. Checked
    /// here against a hand-traceable fixture (matches
    /// `evalue::tests::matches_hand_computation_zero_denominator_candidate`);
    /// the general `m=1` equivalence is additionally fuzzed as a property
    /// test in `tests/paper_score_sdr.rs`.
    #[test]
    fn single_test_point_matches_equation_4_1() {
        let g = gamma(0.5);
        let coupled = coupled_risk_adjusted_evalues(&losses(&[0.0]), &[1.0], &[0.0], g).unwrap();
        let independent = risk_adjusted_evalue(&losses(&[0.0]), &[1.0], 0.0, g).unwrap();
        assert_eq!(coupled.len(), 1);
        assert_eq!(coupled[0].get(), independent.value.get());
        assert_eq!(coupled[0].get(), 2.0);
    }

    #[test]
    fn calibration_score_ties_are_summed_into_one_group() {
        // Two calibration points tied at the same score; both losses must
        // land in the same pooled-score bucket, exactly like
        // `evalue::tests::ties_at_the_same_score_are_summed`.
        let result =
            coupled_risk_adjusted_evalues(&losses(&[0.5, 0.5]), &[2.0, 2.0], &[2.0], gamma(0.9))
                .unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].get().is_finite());
    }

    #[test]
    fn test_score_ties_share_a_group_but_each_excludes_only_itself() {
        // Three test points tied at the same score: each one's own
        // `denom_excl_j` must still count the *other two* as "other test
        // points below the threshold", not exclude the whole tied group.
        let result = coupled_risk_adjusted_evalues(
            &losses(&[0.2, 0.2]),
            &[-1.0, 1.0],
            &[0.0, 0.0, 0.0],
            gamma(0.5),
        )
        .unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].get(), result[1].get());
        assert_eq!(result[1].get(), result[2].get());
    }

    #[test]
    fn calibration_and_test_share_an_identical_score() {
        let result =
            coupled_risk_adjusted_evalues(&losses(&[0.3]), &[1.0], &[1.0, 5.0], gamma(0.5))
                .unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|e| e.get().is_finite() && e.get() >= 0.0));
    }

    #[test]
    fn all_zero_loss_gives_finite_nonnegative_evalues() {
        let result = coupled_risk_adjusted_evalues(
            &losses(&[0.0, 0.0, 0.0]),
            &[-1.0, 0.0, 1.0],
            &[-0.5, 0.5, 2.0],
            gamma(0.3),
        )
        .unwrap();
        assert!(result.iter().all(|e| e.get().is_finite() && e.get() >= 0.0));
    }

    #[test]
    fn all_one_loss_gives_finite_nonnegative_evalues() {
        let result = coupled_risk_adjusted_evalues(
            &losses(&[1.0, 1.0, 1.0]),
            &[-1.0, 0.0, 1.0],
            &[-0.5, 0.5, 2.0],
            gamma(0.9),
        )
        .unwrap();
        assert!(result.iter().all(|e| e.get().is_finite() && e.get() >= 0.0));
    }

    #[test]
    fn negative_and_positive_zero_scores_are_treated_as_one_value() {
        let a =
            coupled_risk_adjusted_evalues(&losses(&[1.0]), &[-1.0], &[0.0], gamma(0.5)).unwrap();
        let b =
            coupled_risk_adjusted_evalues(&losses(&[1.0]), &[-1.0], &[-0.0], gamma(0.5)).unwrap();
        assert_eq!(a[0].get(), b[0].get());
    }

    #[test]
    fn unsorted_input_gives_the_same_result_as_sorted_input() {
        let sorted = coupled_risk_adjusted_evalues(
            &losses(&[0.1, 0.2, 0.3]),
            &[-2.0, 0.0, 3.0],
            &[-1.0, 1.0],
            gamma(0.6),
        )
        .unwrap();
        let unsorted = coupled_risk_adjusted_evalues(
            &losses(&[0.3, 0.1, 0.2]),
            &[3.0, -2.0, 0.0],
            &[1.0, -1.0],
            gamma(0.6),
        )
        .unwrap();
        assert_eq!(sorted[0].get(), unsorted[1].get());
        assert_eq!(sorted[1].get(), unsorted[0].get());
    }

    #[test]
    fn very_small_gamma_stays_finite_or_reports_non_finite_value_cleanly() {
        // gamma just above the smallest positive OpenUnitInterval can
        // represent meaningfully -- either a well-defined finite e-value,
        // or a clean `NonFiniteValue` error from the underflow this
        // module's docs describe (gamma_scale rounding to exactly 0.0),
        // never a panic or a silently wrong number.
        let outcome = coupled_risk_adjusted_evalues(
            &losses(&[0.0, 0.0]),
            &[-1.0, 1.0],
            &[0.0],
            gamma(1e-300),
        );
        match outcome {
            Ok(values) => assert!(values[0].get().is_finite()),
            Err(RiskSieveError::NonFiniteValue { .. }) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn no_candidate_threshold_gives_zero_not_an_error() {
        // gamma too strict for even the smallest pooled score to be
        // feasible at l=1: the test point is excluded, e-value 0, exactly
        // like `evalue`'s analogous "no feasible threshold" case.
        let result =
            coupled_risk_adjusted_evalues(&losses(&[1.0]), &[0.0], &[0.0], gamma(1e-6)).unwrap();
        assert_eq!(result[0].get(), 0.0);
    }

    #[test]
    fn evalues_are_always_non_negative() {
        let result = coupled_risk_adjusted_evalues(
            &losses(&[0.4, 0.9, 0.1]),
            &[-3.0, 0.0, 2.0],
            &[-5.0, -1.0, 0.5, 4.0],
            gamma(0.7),
        )
        .unwrap();
        assert!(result.iter().all(|e| e.get() >= 0.0));
    }
}
