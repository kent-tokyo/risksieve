//! SCoRE-SDR: batch selective deployment controlling Selective Deployment
//! Risk (Algorithm 2, Theorem 3.3).
//!
//! Bai and Jin (2026), arXiv:2603.24704, Algorithm 2 processes a batch of
//! `m` unlabeled test points against one calibration set: compute a
//! per-test-point e-value for each, then apply eBH (Theorem 3.3, see
//! [`super::ebh`]) to the batch to select a set `R` controlling
//!
//! ```text
//! SDR := E[ (sum_{j=1}^m L_{n+j} * 1{j in R}) / (1 v |R|) ] <= alpha
//! ```
//!
//! ## Which e-value construction this module uses, and why
//!
//! The paper's own per-test-point e-value (its Equation 5.1) couples
//! each test point's threshold search to *every other* test point in the
//! batch: its normalizing function divides by `1 + sum_{k != j}
//! 1{s(X_{n+k}) <= t}`, so `E_{n+j}` for one test point depends on all
//! `m` test scores, not just calibration and its own score. The paper's
//! "Algorithm 3" gives an efficient computation for this, but its exact
//! steps were not extractable from this paper's HTML rendering across
//! several independent, targeted fetch attempts (it was truncated every
//! time). Absent that, deriving a correct algorithm independently (the
//! way [`super::evalue`] derives Equation 4.1's infimum) is not
//! straightforward here: that derivation relied on the per-threshold
//! empirical-risk function being non-decreasing in the threshold `t`
//! because it was a fixed-denominator sum; Equation 5.1's normalizing
//! function is a *ratio* of two non-decreasing quantities (a numerator
//! sum and a denominator count, both growing with `t`), and a ratio of
//! two non-decreasing functions is not in general monotonic. Implementing
//! this with the same confidence as Milestone 4 would require either
//! recovering Algorithm 3's exact text or an independent proof of a
//! correct computational method, neither of which has been done.
//!
//! **This module instead reuses [`super::evalue::risk_adjusted_evalue`]
//! independently for each test point in the batch**, exactly as
//! Milestone 4 uses it for a single test point. This remains a fully
//! valid instantiation of Theorem 3.3: that theorem's hypothesis is only
//! that each `E_{n+j}` individually satisfies Definition 3.1, which
//! Theorem 4.2 establishes for `risk_adjusted_evalue`'s construction
//! regardless of which other test points exist. What is lost is
//! *selection power*, not validity -- the paper introduces Equation 5.1
//! because accounting for competing test points is presumably more
//! powerful in a multiple-testing sense, analogous to how Milestone 3's
//! Theorem 1 remains valid for any beta-stable algorithm even though its
//! concrete instances (deferred there too) give tighter bounds. See
//! `tasks/todo.md` for tracking Equation 5.1 itself.
//!
//! ## Assumptions
//!
//! Exchangeability here is required over the *entire* `{(X_i,Y_i)}_{i=1}^{n+m}`
//! set (calibration plus every test point together), not just `n+1` as in
//! [`super::mdr`] -- a strictly stronger assumption. A caller who submits
//! adversarially selected or ordered test points as a batch is relying on
//! this holding across all of them jointly, not on each test point being
//! exchangeable with calibration in isolation.
//!
//! **The score function need not be calibrated or accurate** -- see
//! [`super::evalue`]'s module docs; the same caution about selection
//! power applies here.
//!
//! **Provenance:** this paper postdates this project's training-data
//! cutoff. The SDR definition, Algorithm 2's structure, and Theorem 3.3
//! were cross-checked across independent fetches that agreed
//! digit-for-digit; Equation 5.1 and Algorithm 3 were also confirmed
//! present and consistent across fetches, but their exact computational
//! treatment was not recoverable in enough detail to implement.

use crate::certificate::{Diagnostics, RiskCertificate};
use crate::error::RiskSieveError;
use crate::guarantee::{
    Assumptions, ExchangeabilityAssumption, GuaranteeKind, MonotonicityAssumption, ShiftAssumption,
    StabilityEvidence, SymmetryAssumption,
};
use crate::numerics::summation::kahan_sum;
use crate::probability::{ClosedInterval, ClosedUnitInterval, OpenUnitInterval};
use crate::selective::ebh;
use crate::selective::evalue::risk_adjusted_evalue;

/// Runs Algorithm 2 (SCoRE-SDR) over a batch of `test_scores` and returns
/// the selected-set certificate.
///
/// `parameter` is the sorted-ascending list of selected indices into
/// `test_scores` (AGENTS.md's "deterministic ordering" requirement). An
/// empty result is a valid certificate, not an error -- see
/// [`super::ebh`]'s zero-selection docs.
///
/// # Errors
///
/// Propagated from [`risk_adjusted_evalue`] for each test point: see its
/// documentation for `calibration_losses`/`calibration_scores` length
/// mismatches, an empty calibration set, or a non-finite score.
///
/// # Example
///
/// ```
/// use risksieve::selective::sdr::certify;
/// use risksieve::{ClosedUnitInterval, GuaranteeKind, OpenUnitInterval};
///
/// let losses: Vec<ClosedUnitInterval> = (0..20)
///     .map(|i| ClosedUnitInterval::new("loss", if i % 4 == 0 { 1.0 } else { 0.0 }).unwrap())
///     .collect();
/// let calibration_scores: Vec<f64> = (0..20).map(|i| i as f64).collect();
/// let test_scores = [-5.0, -3.0, 50.0];
/// let alpha = OpenUnitInterval::new("alpha", 0.3)?;
/// let certificate = certify(&losses, &calibration_scores, &test_scores, alpha, alpha)?;
/// assert_eq!(certificate.guarantee, GuaranteeKind::SelectiveDeploymentRisk);
/// # Ok::<(), risksieve::RiskSieveError>(())
/// ```
pub fn certify(
    calibration_losses: &[ClosedUnitInterval],
    calibration_scores: &[f64],
    test_scores: &[f64],
    alpha: OpenUnitInterval,
    gamma: OpenUnitInterval,
) -> Result<RiskCertificate<Vec<usize>>, RiskSieveError> {
    let mut evalues = Vec::with_capacity(test_scores.len());
    for &test_score in test_scores {
        let outcome =
            risk_adjusted_evalue(calibration_losses, calibration_scores, test_score, gamma)?;
        evalues.push(outcome.value);
    }

    let selection = ebh::select(&evalues, alpha);
    let selected_count = selection.selected_indices.len();

    let assumptions = Assumptions {
        exchangeability: ExchangeabilityAssumption::Exchangeable,
        bounded_loss: ClosedInterval::new(0.0, 1.0)?,
        monotonicity: MonotonicityAssumption::NonMonotone,
        right_continuity: false,
        symmetry: SymmetryAssumption::ProvenSymmetric,
        stability: StabilityEvidence::Unknown,
        shift: ShiftAssumption::NoShift,
    };

    Ok(RiskCertificate {
        parameter: selection.selected_indices,
        target_risk: alpha.get(),
        certified_upper_bound: alpha.get(),
        guarantee: GuaranteeKind::SelectiveDeploymentRisk,
        assumptions,
        calibration_size: calibration_losses.len(),
        diagnostics: Diagnostics {
            selected_count: Some(selected_count),
            gamma: Some(gamma.get()),
            ebh_tau_hat: selection.tau_hat,
            uninformative_result: Some(selected_count == 0),
            ..Default::default()
        },
    })
}

/// The *realized* (post-hoc) selective risk among the selected items,
/// once their labels become available.
///
/// This is a descriptive statistic computed from actual outcomes, not a
/// guarantee, and computing it does not validate the certificate that
/// produced the selected set -- `certify`'s `SelectiveDeploymentRisk`
/// bound is about the expectation over the draw, not about any single
/// realized batch (the same distinction [`super::mdr`] draws for MDR).
/// `selected_losses` must be exactly the realized losses for the items in
/// `certify`'s returned `parameter` (in any order); pass an empty slice
/// for zero selections.
///
/// The denominator is `max(1, selected_losses.len())`
/// (AGENTS.md: "Never replace the denominator `max(1, selected_count)`
/// with a different convention") so this is always `0.0`, never `NaN`,
/// when nothing was selected.
pub fn realized_selective_risk(selected_losses: &[ClosedUnitInterval]) -> f64 {
    let numerator = kahan_sum(selected_losses.iter().map(|loss| loss.get()));
    let denominator = selected_losses.len().max(1) as f64;
    numerator / denominator
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
    fn empty_batch_is_a_valid_empty_certificate() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let certificate = certify(&losses(&[1.0]), &[0.0], &[], alpha, alpha).unwrap();
        assert_eq!(certificate.parameter, Vec::<usize>::new());
        assert_eq!(certificate.diagnostics.selected_count, Some(0));
        assert_eq!(certificate.diagnostics.uninformative_result, Some(true));
        assert_eq!(certificate.diagnostics.ebh_tau_hat, None);
        assert_eq!(
            certificate.guarantee,
            GuaranteeKind::SelectiveDeploymentRisk
        );
    }

    #[test]
    fn propagates_errors_from_per_item_evalue_computation() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let err = certify(&losses(&[1.0]), &[0.0, 1.0], &[0.5], alpha, alpha).unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn selects_and_records_tau_hat() {
        // Same calibration fixture as `mdr::tests::deploys_when_evalue_clears_the_threshold`
        // (e-value 2.0 for a test point at score 1.0), repeated across a
        // batch so eBH has something to select.
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let certificate = certify(&losses(&[0.0]), &[1.0], &[0.0, 0.0, 0.0], alpha, alpha).unwrap();
        assert_eq!(certificate.parameter, vec![0, 1, 2]);
        assert_eq!(certificate.diagnostics.selected_count, Some(3));
        assert!(certificate.diagnostics.ebh_tau_hat.is_some());
        assert_eq!(certificate.diagnostics.uninformative_result, Some(false));
    }

    #[test]
    fn realized_selective_risk_of_empty_selection_is_zero_not_nan() {
        assert_eq!(realized_selective_risk(&[]), 0.0);
    }

    #[test]
    fn realized_selective_risk_matches_hand_computation() {
        let selected = losses(&[1.0, 0.0, 0.0, 1.0]);
        assert_eq!(realized_selective_risk(&selected), 0.5);
    }
}
