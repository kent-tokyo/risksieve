//! SCoRE-MDR: the direct deployment decision from a risk-adjusted
//! e-value (Algorithm 1, Theorem 3.2).
//!
//! Bai and Jin (2026), arXiv:2603.24704, Algorithm 1 ("SCoRE-MDR"):
//!
//! ```text
//! Input: labeled calibration {(X_i,Y_i)}_{i=1}^n, test point X_{n+1},
//!        pre-trained score s(.), MDR target alpha in (0,1), gamma in (0,1).
//! 1. Compute calibration risks L_i = loss(f, X_i, Y_i) for i = 1..n.
//! 2. Obtain scores M := {s(X_i)}_{i=1}^{n+1}.
//! 3. Compute E_{gamma,n+1} via Equation (4.1) (see `super::evalue`).
//! 4. Deploy: psi_hat_{n+1} = 1{ E_{gamma,n+1} >= 1/alpha }.
//! ```
//!
//! Theorem 3.2: if `E_{n+1}` obeys Definition 3.1 (which Theorem 4.2
//! establishes for `E_{gamma,n+1}` as constructed above, for *any* fixed
//! `gamma in (0,1)`), then thresholding it this way controls the
//! Marginal Deployment Risk:
//!
//! ```text
//! MDR := E[L_{n+1} * psi_hat_{n+1}] <= alpha
//! ```
//!
//! **This bounds an expectation over the joint draw of the calibration
//! set and the test point, not a property of any single returned
//! decision.** When `certify` returns `parameter: true` (deploy), the
//! realized loss `L_{n+1}` for that specific test point is still unknown
//! and can be anywhere in `[0, 1]` -- the guarantee is that *averaged
//! over repeated draws*, the deployed loss is at most `alpha`. Do not
//! read `certified_upper_bound` as "this deployment has risk `<= alpha`";
//! it is `E[L * psi_hat] <= alpha`, marginal, not conditional on this
//! decision (AGENTS.md section 4: never collapse a guarantee kind into a
//! claim about one realized outcome).
//!
//! ## Implied Total Deployment Risk
//!
//! `TDR := E[sum_{j=1}^m L_{n+j} * psi_hat_{n+j}] <= alpha * m` follows
//! immediately from applying MDR control independently to each of `m`
//! test points at the same `alpha` -- summing `m` copies of
//! `E[L * psi_hat] <= alpha`. It is not a separately computed quantity:
//! there is no batch API in this module yet (that lands with SDR and
//! `eBH` in Milestone 5), so a caller who applies [`certify`] to `m` test
//! points at a fixed `alpha` already has `TDR <= alpha * m` for free.
//!
//! ## Choosing `gamma`
//!
//! Remark 4.5: setting `gamma = alpha` gives optimal asymptotic selection
//! power; `gamma < alpha` is valid but more conservative (fewer
//! deployments); `gamma > alpha` remains valid (Theorem 4.2 holds for any
//! fixed `gamma in (0,1)`) but its power degrades asymptotically to zero
//! without an additional thresholding condition (Theorem 4.6, not
//! implemented here -- see `docs/roadmap.md`). This crate does not default
//! or silently pick `gamma`; pass `alpha` again as `gamma` if unsure.
//!
//! **The score function `s(.)` need not be calibrated or accurate for
//! this theorem** -- see `super::evalue`'s module docs. A poor score
//! can still severely reduce how often a genuinely trustworthy point
//! gets deployed, even though `MDR <= alpha` never breaks.

use crate::certificate::{Diagnostics, RiskCertificate};
use crate::error::RiskSieveError;
use crate::guarantee::{
    Assumptions, ExchangeabilityAssumption, GuaranteeKind, MonotonicityAssumption, ShiftAssumption,
    StabilityEvidence, SymmetryAssumption,
};
use crate::probability::{ClosedInterval, ClosedUnitInterval, OpenUnitInterval};
use crate::selective::evalue::risk_adjusted_evalue;

/// Runs Algorithm 1 (SCoRE-MDR) for one test point and returns its
/// deployment-decision certificate.
///
/// See the module docs for what the returned [`GuaranteeKind::MarginalDeploymentRisk`]
/// certificate does and does not say about this particular decision.
///
/// # Errors
///
/// - [`RiskSieveError::AssumptionMismatch`] if `calibration_losses` and
///   `calibration_scores` have different lengths.
/// - [`RiskSieveError::EmptyCalibrationSet`] if they are empty.
/// - [`RiskSieveError::NonFiniteValue`] if `test_score` or any
///   calibration score is NaN or infinite.
///
/// # Example
///
/// ```
/// use risksieve::selective::mdr::certify;
/// use risksieve::{ClosedUnitInterval, GuaranteeKind, OpenUnitInterval};
///
/// let losses: Vec<ClosedUnitInterval> = (0..20)
///     .map(|i| ClosedUnitInterval::new("loss", if i % 4 == 0 { 1.0 } else { 0.0 }).unwrap())
///     .collect();
/// let scores: Vec<f64> = (0..20).map(|i| i as f64).collect();
/// let alpha = OpenUnitInterval::new("alpha", 0.3)?;
/// // Remark 4.5: gamma = alpha is the recommended default.
/// let certificate = certify(&losses, &scores, -1.0, alpha, alpha)?;
/// assert_eq!(certificate.guarantee, GuaranteeKind::MarginalDeploymentRisk);
/// # Ok::<(), risksieve::RiskSieveError>(())
/// ```
pub fn certify(
    calibration_losses: &[ClosedUnitInterval],
    calibration_scores: &[f64],
    test_score: f64,
    alpha: OpenUnitInterval,
    gamma: OpenUnitInterval,
) -> Result<RiskCertificate<bool>, RiskSieveError> {
    let calibration_size = calibration_losses.len();
    let outcome = risk_adjusted_evalue(calibration_losses, calibration_scores, test_score, gamma)?;
    let deploy = outcome.value.get() >= 1.0 / alpha.get();

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
        parameter: deploy,
        target_risk: alpha.get(),
        certified_upper_bound: alpha.get(),
        guarantee: GuaranteeKind::MarginalDeploymentRisk,
        assumptions,
        calibration_size,
        diagnostics: Diagnostics {
            risk_adjusted_evalue: Some(outcome.value.get()),
            gamma: Some(gamma.get()),
            uninformative_result: Some(!outcome.feasible_threshold_found),
            ..Default::default()
        },
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
        let alpha = OpenUnitInterval::new("alpha", 0.3).unwrap();
        let err = certify(&losses(&[1.0]), &[0.0, 1.0], 0.5, alpha, alpha).unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn deploys_when_test_point_is_excluded_from_the_threshold() {
        // Same fixture as evalue's `matches_hand_computation_test_point_excluded`:
        // the e-value is 0, so deployment never happens for any alpha < 1.
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let certificate = certify(&losses(&[1.0]), &[0.0], 1.0, alpha, alpha).unwrap();
        assert!(!certificate.parameter);
        assert_eq!(certificate.guarantee, GuaranteeKind::MarginalDeploymentRisk);
        assert_eq!(certificate.diagnostics.risk_adjusted_evalue, Some(0.0));
    }

    #[test]
    fn deploys_when_evalue_clears_the_threshold() {
        // Same fixture as evalue's `matches_hand_computation_zero_denominator_candidate`:
        // e-value is 2.0, so 1/alpha <= 2.0 (alpha >= 0.5) deploys.
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let certificate = certify(&losses(&[0.0]), &[1.0], 0.0, alpha, alpha).unwrap();
        assert!(certificate.parameter);
    }

    #[test]
    fn no_feasible_threshold_is_recorded_as_uninformative() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let gamma = OpenUnitInterval::new("gamma", 0.1).unwrap();
        let certificate = certify(&losses(&[1.0]), &[0.0], 0.0, alpha, gamma).unwrap();
        assert_eq!(certificate.diagnostics.uninformative_result, Some(true));
    }

    #[test]
    fn gamma_is_recorded_separately_from_alpha() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let gamma = OpenUnitInterval::new("gamma", 0.2).unwrap();
        let certificate = certify(&losses(&[1.0]), &[0.0], 1.0, alpha, gamma).unwrap();
        assert_eq!(certificate.target_risk, 0.5);
        assert_eq!(certificate.diagnostics.gamma, Some(0.2));
    }

    #[test]
    fn assumptions_claim_no_more_than_algorithm_1_establishes() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let certificate = certify(&losses(&[1.0]), &[0.0], 1.0, alpha, alpha).unwrap();
        assert_eq!(
            certificate.assumptions.monotonicity,
            MonotonicityAssumption::NonMonotone
        );
        assert!(!certificate.assumptions.right_continuity);
        assert_eq!(
            certificate.assumptions.stability,
            StabilityEvidence::Unknown
        );
    }

    // Proposition 4.4 (efficient shortcut, valid for `gamma <= alpha`):
    // the deployment decision reduces to a closed form that avoids
    // computing Equation 4.1's infimum at all. This is the "efficient
    // computation shortcut only after a transparent reference
    // implementation passes tests" requirement in AGENTS.md's Milestone
    // 4 description -- the shortcut is verified here against `certify`'s
    // general computation rather than wired in as a separate code path
    // (see the module docs and `docs/roadmap.md`).
    //
    // `gamma = alpha * u` for `u` a fraction with denominator 16
    // guarantees `gamma <= alpha` by construction; losses are multiples
    // of `0.25` and scores are small integers so the two independently
    // computed decisions land on the same side of the threshold rather
    // than disagreeing over a sub-ULP rounding difference.
    proptest::proptest! {
        #[test]
        fn score_proposition_4_4_shortcut_matches_general_decision(
            pairs in proptest::collection::vec((-20i32..20, 0..5usize), 1..8),
            test_score_int in -20i32..20,
            alpha_num in 1u32..16,
            u_num in 1u32..=16,
        ) {
            let discrete = [0.0, 0.25, 0.5, 0.75, 1.0];
            let scores: Vec<f64> = pairs.iter().map(|&(s, _)| s as f64).collect();
            let loss_values: Vec<f64> = pairs.iter().map(|&(_, li)| discrete[li]).collect();
            let losses: Vec<ClosedUnitInterval> = loss_values
                .iter()
                .map(|&v| ClosedUnitInterval::new("loss", v).unwrap())
                .collect();
            let test_score = test_score_int as f64;

            let alpha = OpenUnitInterval::new("alpha", alpha_num as f64 / 16.0).unwrap();
            let u = u_num as f64 / 16.0;
            let gamma = OpenUnitInterval::new("gamma", alpha.get() * u).unwrap();

            let certificate = certify(&losses, &scores, test_score, alpha, gamma).unwrap();

            let n = losses.len();
            let below_sum: f64 = scores
                .iter()
                .zip(loss_values.iter())
                .filter(|&(&s, _)| s <= test_score)
                .map(|(_, &l)| l)
                .sum();
            let shortcut_average = (1.0 + below_sum) / (n as f64 + 1.0);
            let shortcut_deploy = shortcut_average <= gamma.get();

            proptest::prop_assert_eq!(certificate.parameter, shortcut_deploy);
        }
    }
}
