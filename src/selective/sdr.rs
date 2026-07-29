//! SCoRE-SDR: batch selective deployment controlling Selective Deployment
//! Risk (Algorithm 2, Theorem 3.3).
//!
//! Bai and Jin (2026), *Conformal Selective Prediction with General Risk
//! Control*, arXiv:2603.24704, Algorithm 2 processes a batch of `m`
//! unlabeled test points against one calibration set: compute a
//! per-test-point e-value for each, then apply eBH (Theorem 3.3, see
//! [`super::ebh`]) to the batch to select a set `R` controlling
//!
//! ```text
//! SDR := E[ (sum_{j=1}^m L_{n+j} * 1{j in R}) / (1 v |R|) ] <= alpha
//! ```
//!
//! ## Two e-value constructions, one selection engine
//!
//! [`certify`] uses the paper's own cross-test-point-coupled e-value
//! (Equation 5.1, Theorem 5.1; see [`super::coupled`] for the full
//! derivation) and is the default entry point for this module.
//! [`certify_independent`] instead applies [`super::evalue::risk_adjusted_evalue`]
//! (Equation 4.1) to each test point on its own, ignoring every other test
//! point's score -- this was this crate's only SDR construction before
//! Equation 5.1 was implemented, and remains available under an explicit
//! name because it is simpler to reason about, cheaper (no per-test-point
//! `O(n+m)` scan), and still a fully valid instantiation of Theorem 3.3
//! (whose hypothesis only requires each e-value to individually satisfy
//! Definition 3.1 -- it does not require Equation 5.1's specific
//! construction). Both functions build their final certificate through
//! the same private assembly step, so the eBH selection and certificate
//! fields behave identically regardless of which e-value construction
//! produced the input.
//!
//! **Neither construction is asserted to always dominate the other in
//! selection power.** [`super::coupled`]'s module docs record what a
//! numerical comparison across the shared test fixtures actually found
//! (see also `docs/references.md`); the paper does not prove a general
//! dominance result, so this crate does not claim one either.
//!
//! ## Assumptions
//!
//! Exchangeability here is required over the *entire* `{(X_i,Y_i)}_{i=1}^{n+m}`
//! set (calibration plus every test point together), not just `n+1` as in
//! [`super::mdr`] -- a strictly stronger assumption, and one [`certify`]'s
//! coupled construction actively uses (each test point's e-value depends
//! on every other test point's score), not merely one stated for future
//! extensions. A caller who submits adversarially selected or ordered test
//! points as a batch is relying on this holding across all of them
//! jointly, not on each test point being exchangeable with calibration in
//! isolation.
//!
//! **The score function need not be calibrated or accurate** -- see
//! [`super::evalue`]'s module docs; the same caution about selection power
//! applies here.
//!
//! **Provenance:** this paper postdates this project's training-data
//! cutoff. The SDR definition, Algorithm 2's structure, and Theorem 3.3
//! were cross-checked across independent fetches that agreed
//! digit-for-digit; Equation 5.1, Theorem 5.1, and Algorithm 3's
//! computational structure were independently re-derived and cross-checked
//! against `Tian-Bai/SCoRE`'s `SCoRE_SDR` (see [`super::coupled`] and
//! `THIRD_PARTY_NOTICES.md`), rather than transcribed from a single fetch.

use crate::certificate::{Diagnostics, RiskCertificate};
use crate::error::RiskSieveError;
use crate::guarantee::{
    Assumptions, ExchangeabilityAssumption, GuaranteeKind, MonotonicityAssumption, ShiftAssumption,
    StabilityEvidence, SymmetryAssumption,
};
use crate::numerics::summation::kahan_sum;
use crate::probability::{ClosedInterval, ClosedUnitInterval, NonNegative, OpenUnitInterval};
use crate::selective::coupled::coupled_risk_adjusted_evalues;
use crate::selective::ebh;
use crate::selective::evalue::risk_adjusted_evalue;

fn assemble_certificate(
    evalues: &[NonNegative],
    calibration_size: usize,
    alpha: OpenUnitInterval,
    gamma: OpenUnitInterval,
) -> RiskCertificate<Vec<usize>> {
    let selection = ebh::select(evalues, alpha);
    let selected_count = selection.selected_indices.len();

    let assumptions = Assumptions {
        exchangeability: ExchangeabilityAssumption::Exchangeable,
        bounded_loss: ClosedInterval::new(0.0, 1.0).expect("[0, 1] is a valid closed interval"),
        monotonicity: MonotonicityAssumption::NonMonotone,
        right_continuity: false,
        symmetry: SymmetryAssumption::ProvenSymmetric,
        stability: StabilityEvidence::Unknown,
        shift: ShiftAssumption::NoShift,
    };

    RiskCertificate {
        parameter: selection.selected_indices,
        target_risk: alpha.get(),
        certified_upper_bound: alpha.get(),
        guarantee: GuaranteeKind::SelectiveDeploymentRisk,
        assumptions,
        calibration_size,
        diagnostics: Diagnostics {
            selected_count: Some(selected_count),
            gamma: Some(gamma.get()),
            ebh_tau_hat: selection.tau_hat,
            uninformative_result: Some(selected_count == 0),
            ..Default::default()
        },
    }
}

/// Runs Algorithm 2 (SCoRE-SDR) over a batch of `test_scores` using the
/// paper's own cross-test-point-coupled e-value (Equation 5.1, Theorem
/// 5.1; see [`super::coupled`]) and returns the selected-set certificate.
///
/// `parameter` is the sorted-ascending list of selected indices into
/// `test_scores` (AGENTS.md's "deterministic ordering" requirement). An
/// empty result is a valid certificate, not an error -- see
/// [`super::ebh`]'s zero-selection docs.
///
/// # Errors
///
/// See [`super::coupled::coupled_risk_adjusted_evalues`]: an empty
/// calibration set, mismatched calibration lengths, or a non-finite
/// calibration or test score.
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
    let evalues =
        coupled_risk_adjusted_evalues(calibration_losses, calibration_scores, test_scores, gamma)?;
    Ok(assemble_certificate(
        &evalues,
        calibration_losses.len(),
        alpha,
        gamma,
    ))
}

/// Runs Algorithm 2 (SCoRE-SDR) using [`super::evalue::risk_adjusted_evalue`]
/// (Equation 4.1) applied independently to each test point, ignoring every
/// other test point's score -- this crate's e-value construction before
/// Equation 5.1 was implemented. See the module docs for why this remains
/// available and valid rather than being replaced outright.
///
/// # Errors
///
/// Propagated from [`risk_adjusted_evalue`] for each test point: see its
/// documentation for calibration length mismatches, an empty calibration
/// set, or a non-finite score.
///
/// # Example
///
/// ```
/// use risksieve::selective::sdr::certify_independent;
/// use risksieve::{ClosedUnitInterval, GuaranteeKind, OpenUnitInterval};
///
/// let losses: Vec<ClosedUnitInterval> = (0..20)
///     .map(|i| ClosedUnitInterval::new("loss", if i % 4 == 0 { 1.0 } else { 0.0 }).unwrap())
///     .collect();
/// let calibration_scores: Vec<f64> = (0..20).map(|i| i as f64).collect();
/// let test_scores = [-5.0, -3.0, 50.0];
/// let alpha = OpenUnitInterval::new("alpha", 0.3)?;
/// let certificate = certify_independent(&losses, &calibration_scores, &test_scores, alpha, alpha)?;
/// assert_eq!(certificate.guarantee, GuaranteeKind::SelectiveDeploymentRisk);
/// # Ok::<(), risksieve::RiskSieveError>(())
/// ```
pub fn certify_independent(
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

    Ok(assemble_certificate(
        &evalues,
        calibration_losses.len(),
        alpha,
        gamma,
    ))
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
    fn independent_empty_batch_is_also_a_valid_empty_certificate() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let certificate = certify_independent(&losses(&[1.0]), &[0.0], &[], alpha, alpha).unwrap();
        assert_eq!(certificate.parameter, Vec::<usize>::new());
    }

    #[test]
    fn propagates_errors_from_coupled_evalue_computation() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let err = certify(&losses(&[1.0]), &[0.0, 1.0], &[0.5], alpha, alpha).unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn independent_propagates_errors_from_per_item_evalue_computation() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let err =
            certify_independent(&losses(&[1.0]), &[0.0, 1.0], &[0.5], alpha, alpha).unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn independent_selects_and_records_tau_hat() {
        // Same calibration fixture as `mdr::tests::deploys_when_evalue_clears_the_threshold`
        // (e-value 2.0 for a test point at score 1.0), repeated across a
        // batch so eBH has something to select. This is
        // `certify_independent`'s pre-Equation-5.1 behavior, pinned here
        // under its explicit name.
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let certificate =
            certify_independent(&losses(&[0.0]), &[1.0], &[0.0, 0.0, 0.0], alpha, alpha).unwrap();
        assert_eq!(certificate.parameter, vec![0, 1, 2]);
        assert_eq!(certificate.diagnostics.selected_count, Some(3));
        assert!(certificate.diagnostics.ebh_tau_hat.is_some());
        assert_eq!(certificate.diagnostics.uninformative_result, Some(false));
    }

    #[test]
    fn coupled_selects_and_records_tau_hat_on_the_same_fixture() {
        // The coupled construction on the identical fixture: with three
        // *identical* test points (m=3, all tied at score 0.0), each
        // test point's "other test points below threshold" count is the
        // same for all three by symmetry, so the coupled and independent
        // constructions happen to agree here (worked out in
        // `tests/paper_score_sdr.rs`); this is a coincidence of the tied
        // fixture, not a general equivalence claim.
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let certificate = certify(&losses(&[0.0]), &[1.0], &[0.0, 0.0, 0.0], alpha, alpha).unwrap();
        assert_eq!(certificate.parameter, vec![0, 1, 2]);
        assert_eq!(certificate.diagnostics.selected_count, Some(3));
        assert!(certificate.diagnostics.ebh_tau_hat.is_some());
    }

    #[test]
    fn coupled_and_independent_can_select_different_sets() {
        // Found by random search over the official `Tian-Bai/SCoRE`
        // oracle (`scripts/oracles/generate_score_sdr.py`'s search mode)
        // and independently re-verified against this crate below: the
        // coupled construction's e-value for the low-scoring test point
        // (index 0) is 5.8530875 (computed against `SCoRE_SDR`), large
        // enough to clear the eBH threshold on its own (m=2, alpha=0.519:
        // tau=1 needs 2/(0.519*1)=3.853), while the same test point's
        // independent (Equation 4.1) e-value is only 2.9265437518290893
        // (this crate's own `risk_adjusted_evalue`), below that same
        // threshold -- so the coupled construction selects it and the
        // independent one does not. Index 1's e-value is 0 under both
        // constructions (its own score is too high to be within any
        // feasible threshold). Note: the official `SCoRE_MDR_bf` brute
        // force reports 3.0525030525030523 for this same input, which is
        // wrong -- see `docs/references.md` for why that function is not
        // used as an oracle in this crate.
        let calib_losses = losses(&[0.118, 0.9619, 0.9086, 0.6997, 0.2659]);
        let calib_scores = [2.8151, 1.6725, 1.3013, -0.3038, -1.3666];
        let test_scores = [-2.4217, 2.4156];
        let alpha = OpenUnitInterval::new("alpha", 0.519).unwrap();
        let gamma = OpenUnitInterval::new("gamma", 0.3417).unwrap();

        let coupled = certify(&calib_losses, &calib_scores, &test_scores, alpha, gamma).unwrap();
        let independent =
            certify_independent(&calib_losses, &calib_scores, &test_scores, alpha, gamma).unwrap();

        assert_eq!(coupled.parameter, vec![0]);
        assert_eq!(independent.parameter, Vec::<usize>::new());
        assert_ne!(coupled.parameter, independent.parameter);
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

    // AGENTS.md section 9.3: "permutation invariance of symmetric
    // procedures." The test batch feeding eBH (Theorem 3.3) should select
    // the same set of *identities*, and the same `tau_hat`, regardless of
    // the order test points were submitted in -- eBH's own tie-breaking
    // convention only affects internal sort order, never `tau_hat`'s value
    // or which values clear the final threshold (`ebh.rs`'s own docs and
    // property test cover that in isolation; this checks it survives
    // `certify`'s full batch pipeline, coupled e-value construction
    // included).
    proptest::proptest! {
        #[test]
        fn coupled_certify_selected_set_is_invariant_to_test_batch_order(
            raw_calib in proptest::collection::vec((-5i32..5, 0..5usize), 1..8),
            raw_test in proptest::collection::vec(-5i32..5, 1..8),
            shuffle_keys in proptest::collection::vec(0i32..1000, 1..8),
            alpha_num in 1u32..16,
            gamma_num in 1u32..16,
        ) {
            let discrete = [0.0, 0.25, 0.5, 0.75, 1.0];
            let calib_scores: Vec<f64> = raw_calib.iter().map(|&(s, _)| s as f64).collect();
            let calib_losses: Vec<ClosedUnitInterval> = raw_calib
                .iter()
                .map(|&(_, li)| ClosedUnitInterval::new("loss", discrete[li]).unwrap())
                .collect();
            let test_scores: Vec<f64> = raw_test.iter().map(|&s| s as f64).collect();
            let m = test_scores.len();
            let alpha = OpenUnitInterval::new("alpha", alpha_num as f64 / 16.0).unwrap();
            let gamma = OpenUnitInterval::new("gamma", gamma_num as f64 / 16.0).unwrap();

            let original = certify(&calib_losses, &calib_scores, &test_scores, alpha, gamma).unwrap();
            let mut original_selected: Vec<f64> =
                original.parameter.iter().map(|&i| test_scores[i]).collect();
            original_selected.sort_by(f64::total_cmp);

            let mut order: Vec<usize> = (0..m).collect();
            order.sort_by_key(|&i| shuffle_keys.get(i).copied().unwrap_or(0));
            let permuted_test_scores: Vec<f64> = order.iter().map(|&i| test_scores[i]).collect();

            let permuted =
                certify(&calib_losses, &calib_scores, &permuted_test_scores, alpha, gamma).unwrap();
            let mut permuted_selected: Vec<f64> = permuted
                .parameter
                .iter()
                .map(|&i| permuted_test_scores[i])
                .collect();
            permuted_selected.sort_by(f64::total_cmp);

            proptest::prop_assert_eq!(original.diagnostics.ebh_tau_hat, permuted.diagnostics.ebh_tau_hat);
            proptest::prop_assert_eq!(original_selected, permuted_selected);
        }
    }
}
