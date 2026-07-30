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
//!
//! ## Weighted MDR under covariate shift
//!
//! [`certify_weighted`] extends [`certify`] to Section 6's covariate-shift
//! setting (Equation 6.1, Theorem 6.2, Theorem 6.4; see
//! `super::evalue_weighted`'s module docs for the full derivation and its
//! correspondence to the official `SCoRE_MDR_w`). It takes an explicit
//! [`ImportanceWeightSource`] -- never defaulted -- that determines the
//! returned certificate's [`GuaranteeKind`]:
//!
//! - [`ImportanceWeightSource::KnownDensityRatio`]: the importance weight
//!   is exactly known (Assumption 6.1's `w(.)`), so Theorem 6.2 applies
//!   and the certificate is [`GuaranteeKind::MarginalDeploymentRisk`],
//!   the same finite-sample guarantee kind [`certify`] returns.
//! - [`ImportanceWeightSource::Estimated`]: the weight was estimated by a
//!   model trained independent of the calibration data used here (the
//!   caller declares this via `training_data_separate_from_calibration`,
//!   exactly the hypothesis Theorem 6.4 requires). Theorem 6.4's
//!   conclusion is `limsup_{n->infinity} MDR_n <= alpha` -- a limiting
//!   statement, not a finite-sample one -- so the certificate is
//!   [`GuaranteeKind::Asymptotic`] instead, the same downgrade pattern
//!   [`crate::anytime::shifted::AnytimeShiftedController`] applies for
//!   Theorem 4.7's estimated-weight case. Declaring `KnownDensityRatio`
//!   is the caller's assertion, not something this crate verifies from
//!   data (AGENTS.md section 4: caller-declared assumptions are recorded
//!   as such, never silently upgraded to "library-checked").
//!
//! Weights carry no normalization requirement and are invariant to a
//! *uniform* positive rescaling of every weight together (calibration
//! *and* the test point) -- see `super::evalue_weighted`'s module docs.
//! `certify_weighted`'s `Assumptions::exchangeability` is
//! `ExchangeabilityAssumption::Iid` (Assumption 6.1 states i.i.d. within
//! each of the calibration and test distributions, not mere
//! exchangeability), matching how
//! [`crate::anytime::shifted::AnytimeShiftedController`] records the
//! analogous assumption for Theorem 4.7.
//!
//! **Not implemented in this module:** Remark 6.6's "doubly robust"
//! refinement (asymptotic MDR control from a finite-sample balancing
//! condition even when only the weights *or* only a conditional-risk
//! model is consistent, not both) requires additional estimator
//! machinery the paper defers to an appendix; see `docs/roadmap.md`.

use crate::certificate::{Diagnostics, EValue, RiskCertificate};
use crate::error::RiskSieveError;
use crate::guarantee::{
    Assumptions, ExchangeabilityAssumption, GuaranteeKind, ImportanceWeightSource,
    MonotonicityAssumption, ShiftAssumption, StabilityEvidence, SymmetryAssumption,
};
use crate::probability::{ClosedInterval, ClosedUnitInterval, NonNegative, OpenUnitInterval};
use crate::selective::evalue::risk_adjusted_evalue;
use crate::selective::evalue_weighted::weighted_risk_adjusted_evalue;
use crate::shift::importance::WeightAccumulator;

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
            risk_adjusted_evalue: Some(EValue::Finite(outcome.value)),
            gamma: Some(gamma.get()),
            uninformative_result: Some(!outcome.feasible_threshold_found),
            ..Default::default()
        },
    })
}

/// Runs the weighted extension of Algorithm 1 (Equation 6.1, Theorem 6.2 /
/// 6.4) for one test point under covariate shift and returns its
/// deployment-decision certificate.
///
/// See the module docs ("Weighted MDR under covariate shift") for how
/// `weight_source` determines the returned `GuaranteeKind`, and
/// `super::evalue_weighted`'s module docs for the full derivation.
///
/// # Errors
///
/// - [`RiskSieveError::AssumptionMismatch`] if `calibration_losses`,
///   `calibration_scores`, and `calibration_weights` do not all have the
///   same length.
/// - [`RiskSieveError::EmptyCalibrationSet`] if they are empty.
/// - [`RiskSieveError::NonFiniteValue`] if `test_score` or any
///   calibration score is NaN or infinite.
/// - [`RiskSieveError::DegenerateWeights`] if every calibration weight
///   and `test_weight` are exactly zero.
///
/// # Example
///
/// ```
/// use risksieve::selective::mdr::certify_weighted;
/// use risksieve::{
///     ClosedUnitInterval, GuaranteeKind, ImportanceWeightSource, NonNegative, OpenUnitInterval,
/// };
///
/// let losses: Vec<ClosedUnitInterval> = (0..20)
///     .map(|i| ClosedUnitInterval::new("loss", if i % 4 == 0 { 1.0 } else { 0.0 }).unwrap())
///     .collect();
/// let scores: Vec<f64> = (0..20).map(|i| i as f64).collect();
/// let weights: Vec<NonNegative> = (0..20).map(|_| NonNegative::new("weight", 1.0).unwrap()).collect();
/// let alpha = OpenUnitInterval::new("alpha", 0.3)?;
/// let certificate = certify_weighted(
///     &losses,
///     &scores,
///     &weights,
///     -1.0,
///     NonNegative::new("weight", 1.0)?,
///     alpha,
///     alpha,
///     ImportanceWeightSource::KnownDensityRatio,
/// )?;
/// assert_eq!(certificate.guarantee, GuaranteeKind::MarginalDeploymentRisk);
/// # Ok::<(), risksieve::RiskSieveError>(())
/// ```
#[allow(clippy::too_many_arguments)]
pub fn certify_weighted(
    calibration_losses: &[ClosedUnitInterval],
    calibration_scores: &[f64],
    calibration_weights: &[NonNegative],
    test_score: f64,
    test_weight: NonNegative,
    alpha: OpenUnitInterval,
    gamma: OpenUnitInterval,
    weight_source: ImportanceWeightSource,
) -> Result<RiskCertificate<bool>, RiskSieveError> {
    let calibration_size = calibration_losses.len();
    let outcome = weighted_risk_adjusted_evalue(
        calibration_losses,
        calibration_scores,
        calibration_weights,
        test_score,
        test_weight,
        gamma,
    )?;
    let deploy = outcome.value.clears_deployment_threshold(alpha);

    let guarantee = match &weight_source {
        ImportanceWeightSource::KnownDensityRatio => GuaranteeKind::MarginalDeploymentRisk,
        ImportanceWeightSource::Estimated { .. } => GuaranteeKind::Asymptotic,
    };

    let assumptions = Assumptions {
        exchangeability: ExchangeabilityAssumption::Iid,
        bounded_loss: ClosedInterval::new(0.0, 1.0)?,
        monotonicity: MonotonicityAssumption::NonMonotone,
        right_continuity: false,
        symmetry: SymmetryAssumption::ProvenSymmetric,
        stability: StabilityEvidence::Unknown,
        shift: ShiftAssumption::CovariateShift { weight_source },
    };

    let mut weight_stats = WeightAccumulator::new();
    for &w in calibration_weights {
        weight_stats.update(w);
    }

    Ok(RiskCertificate {
        parameter: deploy,
        target_risk: alpha.get(),
        certified_upper_bound: alpha.get(),
        guarantee,
        assumptions,
        calibration_size,
        diagnostics: Diagnostics {
            risk_adjusted_evalue: Some(outcome.value),
            gamma: Some(gamma.get()),
            uninformative_result: Some(!outcome.feasible_threshold_found),
            weight_sum: Some(weight_stats.sum()),
            weight_sum_of_squares: Some(weight_stats.sum_of_squares()),
            effective_sample_size: Some(weight_stats.effective_sample_size()),
            weight_range: weight_stats.range(),
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

    fn finite(v: f64) -> EValue {
        EValue::Finite(NonNegative::new("e", v).unwrap())
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
        assert_eq!(
            certificate.diagnostics.risk_adjusted_evalue,
            Some(finite(0.0))
        );
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

    fn weights(values: &[f64]) -> Vec<NonNegative> {
        values
            .iter()
            .map(|&v| NonNegative::new("weight", v).unwrap())
            .collect()
    }

    fn weight(v: f64) -> NonNegative {
        NonNegative::new("weight", v).unwrap()
    }

    #[test]
    fn weighted_rejects_mismatched_lengths() {
        let alpha = OpenUnitInterval::new("alpha", 0.3).unwrap();
        let err = certify_weighted(
            &losses(&[1.0]),
            &[0.0, 1.0],
            &weights(&[1.0]),
            0.5,
            weight(1.0),
            alpha,
            alpha,
            ImportanceWeightSource::KnownDensityRatio,
        )
        .unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn weighted_rejects_degenerate_combined_weights() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let err = certify_weighted(
            &losses(&[1.0]),
            &[0.0],
            &weights(&[0.0]),
            0.0,
            weight(0.0),
            alpha,
            alpha,
            ImportanceWeightSource::KnownDensityRatio,
        )
        .unwrap_err();
        assert!(matches!(err, RiskSieveError::DegenerateWeights));
    }

    // Same two `n=1` hand fixtures as `certify`'s own tests
    // (`deploys_when_test_point_is_excluded_from_the_threshold` and
    // `deploys_when_evalue_clears_the_threshold`), with every weight set
    // to `1.0`: Equation 6.1 reduces algebraically to Equation 4.1 at
    // uniform weight `1`, so the weighted decision must match the
    // unweighted one exactly (see `evalue_weighted.rs`'s module docs).
    #[test]
    fn weighted_matches_unweighted_decision_when_all_weights_are_one() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let excluded = certify_weighted(
            &losses(&[1.0]),
            &[0.0],
            &weights(&[1.0]),
            1.0,
            weight(1.0),
            alpha,
            alpha,
            ImportanceWeightSource::KnownDensityRatio,
        )
        .unwrap();
        assert!(!excluded.parameter);
        assert_eq!(excluded.diagnostics.risk_adjusted_evalue, Some(finite(0.0)));

        let cleared = certify_weighted(
            &losses(&[0.0]),
            &[1.0],
            &weights(&[1.0]),
            0.0,
            weight(1.0),
            alpha,
            alpha,
            ImportanceWeightSource::KnownDensityRatio,
        )
        .unwrap();
        assert!(cleared.parameter);
    }

    #[test]
    fn known_density_ratio_yields_marginal_deployment_risk() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let certificate = certify_weighted(
            &losses(&[0.0]),
            &[1.0],
            &weights(&[1.0]),
            0.0,
            weight(1.0),
            alpha,
            alpha,
            ImportanceWeightSource::KnownDensityRatio,
        )
        .unwrap();
        assert_eq!(certificate.guarantee, GuaranteeKind::MarginalDeploymentRisk);
        assert_eq!(
            certificate.assumptions.shift,
            ShiftAssumption::CovariateShift {
                weight_source: ImportanceWeightSource::KnownDensityRatio
            }
        );
    }

    #[test]
    fn estimated_weight_source_downgrades_to_asymptotic() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let weight_source = ImportanceWeightSource::Estimated {
            method: "test fixture".to_string(),
            training_data_separate_from_calibration: true,
        };
        let certificate = certify_weighted(
            &losses(&[0.0]),
            &[1.0],
            &weights(&[1.0]),
            0.0,
            weight(1.0),
            alpha,
            alpha,
            weight_source,
        )
        .unwrap();
        assert_eq!(certificate.guarantee, GuaranteeKind::Asymptotic);
    }

    #[test]
    fn weighted_records_weight_diagnostics() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let certificate = certify_weighted(
            &losses(&[0.0, 1.0]),
            &[1.0, 2.0],
            &weights(&[1.0, 3.0]),
            0.0,
            weight(1.0),
            alpha,
            alpha,
            ImportanceWeightSource::KnownDensityRatio,
        )
        .unwrap();
        assert_eq!(certificate.diagnostics.weight_sum, Some(4.0));
        assert_eq!(certificate.diagnostics.weight_sum_of_squares, Some(10.0));
        assert_eq!(certificate.diagnostics.weight_range, Some((1.0, 3.0)));
    }

    #[test]
    fn weighted_exchangeability_is_iid_not_merely_exchangeable() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let certificate = certify_weighted(
            &losses(&[0.0]),
            &[1.0],
            &weights(&[1.0]),
            0.0,
            weight(1.0),
            alpha,
            alpha,
            ImportanceWeightSource::KnownDensityRatio,
        )
        .unwrap();
        assert_eq!(
            certificate.assumptions.exchangeability,
            ExchangeabilityAssumption::Iid
        );
    }
}
