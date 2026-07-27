//! The general symmetry + beta-stability risk-control reduction.
//!
//! Implements Angelopoulos (2026), *Conformal Risk Control for
//! Non-Monotonic Losses*, arXiv:2602.20151, Theorem 1: a generic
//! algorithm `A`, applied to `n` calibration points, controls expected
//! risk on a fresh test point whenever `A` is symmetric
//! (permutation-invariant) and beta-stable with respect to a reference
//! algorithm `A*` whose own expected risk on the full `(n+1)`-point
//! oracle dataset is at most `alpha - beta`:
//!
//! ```text
//! Assume A is symmetric and beta-stable with respect to A*, that
//! D_{1:n+1} is exchangeable, and that
//!     E[loss(X_{n+1}, Y_{n+1}; A*(D_{1:n+1}))] <= alpha - beta.
//! Then E[loss(X_{n+1}, Y_{n+1}; A(D_{1:n}))] <= alpha.
//! ```
//!
//! Unlike [`crate::crc::monotone::certify`] and
//! [`crate::anytime::AnytimeController`], this function does not itself
//! search for a parameter: Theorem 1 is a theorem about an arbitrary
//! algorithm `A`, so the caller supplies the parameter `A` already
//! produced (by whatever optimization procedure they use) along with the
//! stability and reference-risk assumptions, and this function checks
//! those assumptions and assembles the certificate. AGENTS.md section 7:
//! "The optimizer interface must be permutation-invariant or explicitly
//! state that symmetry is the caller's responsibility" — there is
//! deliberately no generic optimizer trait here. A side effect: because
//! `Parameter` carries no trait bounds at all (no search or ordering is
//! performed on it), multidimensional parameters are already supported
//! without extra work.
//!
//! **Scope of this module:** only Theorem 1 itself. The paper's concrete
//! stability instances (discretized bounded losses, continuous Lipschitz
//! losses, selective classification, regularized ERM) each derive their
//! own `beta` from a specific construction and are not implemented yet;
//! see `tasks/todo.md`. In particular, the discretized-loss construction
//! (the paper's Proposition 2) turned out to carry an asymptotic
//! Lambert-W-function bound rather than a clean finite-sample `beta`,
//! and was deferred rather than transcribed with unverified confidence.
//!
//! **Provenance:** this paper postdates this project's training-data
//! cutoff. Theorem 1's statement above was cross-checked across multiple
//! independent fetches of the paper's own text and is stated as an exact
//! (non-asymptotic) result, unlike Proposition 2.
//!
//! Proposition 1 in the source paper shows that classical monotone CRC
//! (Milestone 1, [`crate::crc::monotone`]) is the `beta = 0` special
//! case of this theorem. Feeding `crc::monotone::certify`'s own output
//! back into this function only exercises this function's plumbing (its
//! `parameter`, `target_risk`, and `certified_upper_bound` are passed
//! straight through), so that is not how it is checked. Instead,
//! `tests/paper_nonmonotone.rs`'s `nonmonotone_proposition_1_*` tests
//! validate the claim against a reference algorithm `A*` of this crate's
//! own choosing (the paper's text never named which `A*` it meant
//! precisely enough to transcribe): the natural uncorrected oracle
//! threshold on the full `(n+1)`-point dataset. Under that choice,
//! `beta = 0` is an exact per-dataset domination, not a Monte Carlo
//! estimate — see that file for the derivation.

use crate::certificate::{Diagnostics, RiskCertificate};
use crate::error::RiskSieveError;
use crate::guarantee::{
    Assumptions, ExchangeabilityAssumption, GuaranteeKind, MonotonicityAssumption, ShiftAssumption,
    StabilityEvidence, SymmetryAssumption,
};
use crate::probability::{ClosedInterval, OpenUnitInterval};

/// Certifies `parameter` (already produced by the caller's algorithm `A`)
/// under Theorem 1's symmetry + beta-stability reduction.
///
/// # Assumptions required
///
/// - `symmetry` must be [`SymmetryAssumption::ProvenSymmetric`] or
///   [`SymmetryAssumption::CallerAsserted`]; `NotEstablished` means
///   Theorem 1's hypothesis is not met.
/// - `stability` must carry a beta value ([`StabilityEvidence::Analytic`],
///   [`StabilityEvidence::UserSupplied`], or
///   [`StabilityEvidence::Estimated`]); [`StabilityEvidence::Unknown`] is
///   rejected outright (AGENTS.md section 6.4).
/// - `reference_risk_bound <= target_risk - beta`: the reference
///   algorithm `A*`'s own expected risk on the oracle `(n+1)`-point
///   dataset, which the caller asserts or proves separately from this
///   function.
/// - `calibration_size` and the test point are exchangeable.
///
/// # Guarantee
///
/// - With [`StabilityEvidence::Analytic`] or
///   [`StabilityEvidence::UserSupplied`] (and symmetry established):
///   [`GuaranteeKind::ExpectedRisk`], exactly as Theorem 1 states it — an
///   exact finite-sample bound, not an asymptotic approximation.
/// - With [`StabilityEvidence::Estimated`]: [`GuaranteeKind::EmpiricalOnly`],
///   since no theorem in scope justifies plugging an estimated beta into
///   an exact finite-sample bound (AGENTS.md section 6.4).
///
/// # Errors
///
/// - [`RiskSieveError::EmptyCalibrationSet`] if `calibration_size == 0`.
/// - [`RiskSieveError::MissingStabilityEvidence`] if `stability` is
///   [`StabilityEvidence::Unknown`].
/// - [`RiskSieveError::AssumptionMismatch`] if `symmetry` is
///   [`SymmetryAssumption::NotEstablished`], or if
///   `reference_risk_bound > target_risk - beta`.
/// - [`RiskSieveError::NonFiniteValue`] if `reference_risk_bound` is NaN
///   or infinite.
///
/// # Example
///
/// ```
/// use risksieve::nonmonotone::stability::certify;
/// use risksieve::{ClosedInterval, NonNegative, OpenUnitInterval, StabilityEvidence, SymmetryAssumption};
///
/// let alpha = OpenUnitInterval::new("alpha", 0.1)?;
/// let beta = NonNegative::new("beta", 0.01)?;
/// let certificate = certify(
///     vec![0.5, -1.2, 3.0], // a multidimensional parameter, e.g. a model's weights
///     alpha,
///     alpha.get() - beta.get(), // the tightest valid reference bound
///     ClosedInterval::new(0.0, 1.0)?,
///     StabilityEvidence::Analytic { beta, reference: "worked example".to_string() },
///     SymmetryAssumption::ProvenSymmetric,
///     200,
/// )?;
/// assert_eq!(certificate.parameter, vec![0.5, -1.2, 3.0]);
/// # Ok::<(), risksieve::RiskSieveError>(())
/// ```
pub fn certify<Parameter>(
    parameter: Parameter,
    target_risk: OpenUnitInterval,
    reference_risk_bound: f64,
    loss_bounds: ClosedInterval,
    stability: StabilityEvidence,
    symmetry: SymmetryAssumption,
    calibration_size: usize,
) -> Result<RiskCertificate<Parameter>, RiskSieveError> {
    if calibration_size == 0 {
        return Err(RiskSieveError::EmptyCalibrationSet);
    }
    if matches!(symmetry, SymmetryAssumption::NotEstablished) {
        return Err(RiskSieveError::AssumptionMismatch {
            detail: "Theorem 1 requires a symmetric (permutation-invariant) algorithm".to_string(),
        });
    }
    if !reference_risk_bound.is_finite() {
        return Err(RiskSieveError::NonFiniteValue {
            name: "reference_risk_bound",
            value: reference_risk_bound,
        });
    }

    let beta = match &stability {
        StabilityEvidence::Unknown => return Err(RiskSieveError::MissingStabilityEvidence),
        StabilityEvidence::Analytic { beta, .. } => beta.get(),
        StabilityEvidence::UserSupplied { beta, .. } => beta.get(),
        StabilityEvidence::Estimated { estimate, .. } => estimate.get(),
    };

    let feasible_bound = target_risk.get() - beta;
    if reference_risk_bound > feasible_bound {
        return Err(RiskSieveError::AssumptionMismatch {
            detail: format!(
                "reference_risk_bound ({reference_risk_bound}) must be at most \
                 target_risk - beta ({feasible_bound}); Theorem 1's hypothesis is not met"
            ),
        });
    }

    let guarantee = if matches!(stability, StabilityEvidence::Estimated { .. }) {
        GuaranteeKind::EmpiricalOnly
    } else {
        GuaranteeKind::ExpectedRisk
    };

    let assumptions = Assumptions {
        exchangeability: ExchangeabilityAssumption::Exchangeable,
        bounded_loss: loss_bounds,
        monotonicity: MonotonicityAssumption::NonMonotone,
        right_continuity: false,
        symmetry,
        stability,
        shift: ShiftAssumption::NoShift,
    };

    Ok(RiskCertificate {
        parameter,
        target_risk: target_risk.get(),
        certified_upper_bound: target_risk.get(),
        guarantee,
        assumptions,
        calibration_size,
        diagnostics: Diagnostics {
            stability_beta: Some(beta),
            asserted_reference_bound: Some(reference_risk_bound),
            ..Default::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probability::NonNegative;

    fn analytic(beta: f64) -> StabilityEvidence {
        StabilityEvidence::Analytic {
            beta: NonNegative::new("beta", beta).unwrap(),
            reference: "test fixture".to_string(),
        }
    }

    #[test]
    fn rejects_empty_calibration() {
        let alpha = OpenUnitInterval::new("alpha", 0.1).unwrap();
        let err = certify(
            1.0,
            alpha,
            0.05,
            ClosedInterval::new(0.0, 1.0).unwrap(),
            analytic(0.0),
            SymmetryAssumption::ProvenSymmetric,
            0,
        )
        .unwrap_err();
        assert!(matches!(err, RiskSieveError::EmptyCalibrationSet));
    }

    #[test]
    fn rejects_unestablished_symmetry() {
        let alpha = OpenUnitInterval::new("alpha", 0.1).unwrap();
        let err = certify(
            1.0,
            alpha,
            0.05,
            ClosedInterval::new(0.0, 1.0).unwrap(),
            analytic(0.0),
            SymmetryAssumption::NotEstablished,
            10,
        )
        .unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn rejects_unknown_stability() {
        let alpha = OpenUnitInterval::new("alpha", 0.1).unwrap();
        let err = certify(
            1.0,
            alpha,
            0.05,
            ClosedInterval::new(0.0, 1.0).unwrap(),
            StabilityEvidence::Unknown,
            SymmetryAssumption::ProvenSymmetric,
            10,
        )
        .unwrap_err();
        assert!(matches!(err, RiskSieveError::MissingStabilityEvidence));
    }

    #[test]
    fn rejects_reference_bound_that_violates_the_hypothesis() {
        let alpha = OpenUnitInterval::new("alpha", 0.1).unwrap();
        // alpha - beta = 0.05, so a reference bound of 0.06 is infeasible.
        let err = certify(
            1.0,
            alpha,
            0.06,
            ClosedInterval::new(0.0, 1.0).unwrap(),
            analytic(0.05),
            SymmetryAssumption::ProvenSymmetric,
            10,
        )
        .unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn estimated_stability_downgrades_to_empirical_only() {
        let alpha = OpenUnitInterval::new("alpha", 0.1).unwrap();
        let estimated = StabilityEvidence::Estimated {
            estimate: NonNegative::new("beta", 0.02).unwrap(),
            method: crate::guarantee::StabilityEstimationMethod::Bootstrap { resamples: 500 },
            confidence_interval: None,
        };
        let certificate = certify(
            1.0,
            alpha,
            0.05,
            ClosedInterval::new(0.0, 1.0).unwrap(),
            estimated,
            SymmetryAssumption::ProvenSymmetric,
            10,
        )
        .unwrap();
        assert_eq!(certificate.guarantee, GuaranteeKind::EmpiricalOnly);
        assert_eq!(certificate.diagnostics.stability_beta, Some(0.02));
    }

    #[test]
    fn multidimensional_parameter_requires_no_extra_bounds() {
        let alpha = OpenUnitInterval::new("alpha", 0.1).unwrap();
        let certificate = certify(
            vec![1.0, 2.0, 3.0],
            alpha,
            0.05,
            ClosedInterval::new(0.0, 1.0).unwrap(),
            analytic(0.05),
            SymmetryAssumption::ProvenSymmetric,
            10,
        )
        .unwrap();
        assert_eq!(certificate.parameter, vec![1.0, 2.0, 3.0]);
    }

    /// The reference-risk-bound assumption Theorem 1's hypothesis rests
    /// on must be recoverable from the certificate, not just consumed
    /// internally and discarded (a certificate a reviewer cannot audit
    /// back to its hypothesis is not distinguishable from one with a
    /// weaker or absent assumption; AGENTS.md section 16).
    #[test]
    fn asserted_reference_bound_is_recorded_in_diagnostics() {
        let alpha = OpenUnitInterval::new("alpha", 0.1).unwrap();
        let certificate = certify(
            1.0,
            alpha,
            0.04,
            ClosedInterval::new(0.0, 1.0).unwrap(),
            analytic(0.05),
            SymmetryAssumption::ProvenSymmetric,
            10,
        )
        .unwrap();
        assert_eq!(certificate.diagnostics.asserted_reference_bound, Some(0.04));
    }
}
