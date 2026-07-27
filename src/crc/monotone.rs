//! Classical monotone conformal risk control.
//!
//! Implements the finite-sample expected-risk controller from
//! Angelopoulos, Bates, Fisch, Lei, and Schuster (2024), *Conformal Risk
//! Control*, ICLR 2024, arXiv:2208.02814, Theorem 1.
//!
//! This is the control implementation against which later, more general
//! controllers (anytime-valid, non-monotonic, SCoRE) are compared, so it
//! is intentionally simple: a linear scan over a caller-supplied,
//! ascending candidate grid rather than an optimized search. AGENTS.md
//! section 8: "Keep reference implementations simple and auditable
//! before optimizing."

use crate::certificate::{Diagnostics, RiskCertificate};
use crate::error::RiskSieveError;
use crate::guarantee::{
    Assumptions, ExchangeabilityAssumption, GuaranteeKind, MonotonicityAssumption, ShiftAssumption,
    StabilityEvidence, SymmetryAssumption,
};
use crate::loss::BoundedLoss;
use crate::numerics::summation::kahan_mean;
use crate::probability::OpenUnitInterval;

/// Certifies the smallest candidate parameter whose finite-sample
/// corrected empirical risk is at most `alpha`.
///
/// # Assumptions required
///
/// - `loss.bounds()` is `[0, B]` for some `B >= 0`. A nonzero lower bound
///   is rejected: the cited theorem assumes `L_i(lambda) in [0, B]`, and
///   generalizing to an arbitrary interval would silently change what
///   `alpha` means.
/// - `candidates` is non-empty and sorted in non-decreasing order, and
///   `loss` is non-increasing in the parameter over that order. This is
///   caller-declared: a finite candidate set cannot certify monotonicity
///   of an arbitrary black-box loss.
/// - `calibration` and the eventual test point are exchangeable.
///
/// # Guarantee
///
/// Under the above, `E[L_{n+1}(lambda_hat)] <= alpha`
/// ([`GuaranteeKind::ExpectedRisk`]), where `n = calibration.len()`.
///
/// # Errors
///
/// - [`RiskSieveError::EmptyCalibrationSet`] if `calibration` is empty.
/// - [`RiskSieveError::AssumptionMismatch`] if `candidates` is empty or
///   not sorted, or `loss.bounds().lower() != 0.0`.
/// - [`RiskSieveError::NoFeasibleParameter`] if no candidate meets the
///   corrected risk target; extend `candidates` toward the conservative
///   end of the parameter range if this occurs.
///
/// # Example
///
/// ```
/// use risksieve::crc::monotone::certify;
/// use risksieve::{BoundedLoss, ClosedInterval, OpenUnitInterval, RiskSieveError};
///
/// struct ExceedsThreshold;
/// impl BoundedLoss<f64, f64> for ExceedsThreshold {
///     fn bounds(&self) -> ClosedInterval {
///         ClosedInterval::new(0.0, 1.0).unwrap()
///     }
///     fn evaluate(&self, observation: &f64, parameter: &f64) -> Result<f64, RiskSieveError> {
///         Ok(if observation > parameter { 1.0 } else { 0.0 })
///     }
/// }
///
/// let calibration = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
/// let candidates = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
/// let alpha = OpenUnitInterval::new("alpha", 0.3)?;
///
/// let certificate = certify(&ExceedsThreshold, alpha, &calibration, &candidates)?;
/// assert_eq!(certificate.parameter, 0.8);
/// # Ok::<(), RiskSieveError>(())
/// ```
pub fn certify<L, Observation, Parameter>(
    loss: &L,
    alpha: OpenUnitInterval,
    calibration: &[Observation],
    candidates: &[Parameter],
) -> Result<RiskCertificate<Parameter>, RiskSieveError>
where
    L: BoundedLoss<Observation, Parameter>,
    Parameter: Clone + PartialOrd,
{
    let n = calibration.len();
    if n == 0 {
        return Err(RiskSieveError::EmptyCalibrationSet);
    }
    if candidates.is_empty() {
        return Err(RiskSieveError::AssumptionMismatch {
            detail: "candidates must be non-empty".to_string(),
        });
    }
    if !candidates.is_sorted() {
        return Err(RiskSieveError::AssumptionMismatch {
            detail: "candidates must be sorted in non-decreasing order".to_string(),
        });
    }

    let bounds = loss.bounds();
    if bounds.lower() != 0.0 {
        return Err(RiskSieveError::AssumptionMismatch {
            detail: format!(
                "monotone CRC requires a loss bounded in [0, B]; got lower bound {}",
                bounds.lower()
            ),
        });
    }
    let b = bounds.upper();

    // Theorem 1: lambda_hat = inf { lambda : (n/(n+1)) R_hat(lambda) + B/(n+1) <= alpha },
    // equivalently R_hat(lambda) <= alpha + (alpha - B) / n.
    let corrected_target = alpha.get() + (alpha.get() - b) / n as f64;

    let last_index = candidates.len() - 1;
    let mut losses = Vec::with_capacity(n);
    for (index, candidate) in candidates.iter().enumerate() {
        losses.clear();
        for observation in calibration {
            losses.push(loss.evaluate_checked(observation, candidate)?);
        }
        let empirical_risk = kahan_mean(&losses);
        if empirical_risk <= corrected_target {
            let diagnostics = Diagnostics {
                empirical_risk: Some(empirical_risk),
                correction_term: Some(alpha.get() - corrected_target),
                uninformative_result: Some(index == last_index),
                ..Default::default()
            };
            let assumptions = Assumptions {
                exchangeability: ExchangeabilityAssumption::Exchangeable,
                bounded_loss: bounds,
                monotonicity: MonotonicityAssumption::Monotone {
                    non_increasing: true,
                },
                right_continuity: true,
                symmetry: SymmetryAssumption::NotEstablished,
                stability: StabilityEvidence::Unknown,
                shift: ShiftAssumption::NoShift,
            };
            return Ok(RiskCertificate {
                parameter: candidate.clone(),
                target_risk: alpha.get(),
                certified_upper_bound: alpha.get(),
                guarantee: GuaranteeKind::ExpectedRisk,
                assumptions,
                calibration_size: n,
                diagnostics,
            });
        }
    }

    Err(RiskSieveError::NoFeasibleParameter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probability::ClosedInterval;

    struct ExceedsThreshold;
    impl BoundedLoss<f64, f64> for ExceedsThreshold {
        fn bounds(&self) -> ClosedInterval {
            ClosedInterval::new(0.0, 1.0).unwrap()
        }
        fn evaluate(&self, observation: &f64, parameter: &f64) -> Result<f64, RiskSieveError> {
            Ok(if observation > parameter { 1.0 } else { 0.0 })
        }
    }

    #[test]
    fn rejects_empty_calibration_set() {
        let alpha = OpenUnitInterval::new("alpha", 0.3).unwrap();
        let err = certify(&ExceedsThreshold, alpha, &[] as &[f64], &[0.0, 1.0]).unwrap_err();
        assert!(matches!(err, RiskSieveError::EmptyCalibrationSet));
    }

    #[test]
    fn rejects_empty_candidates() {
        let alpha = OpenUnitInterval::new("alpha", 0.3).unwrap();
        let err = certify(&ExceedsThreshold, alpha, &[0.5], &[] as &[f64]).unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn rejects_unsorted_candidates() {
        let alpha = OpenUnitInterval::new("alpha", 0.3).unwrap();
        let err = certify(&ExceedsThreshold, alpha, &[0.5], &[1.0, 0.0]).unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn rejects_nonzero_lower_bound() {
        struct ShiftedLoss;
        impl BoundedLoss<f64, f64> for ShiftedLoss {
            fn bounds(&self) -> ClosedInterval {
                ClosedInterval::new(1.0, 2.0).unwrap()
            }
            fn evaluate(&self, _o: &f64, _p: &f64) -> Result<f64, RiskSieveError> {
                Ok(1.5)
            }
        }
        let alpha = OpenUnitInterval::new("alpha", 0.3).unwrap();
        let err = certify(&ShiftedLoss, alpha, &[0.5], &[0.0, 1.0]).unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn no_feasible_parameter_when_even_the_conservative_candidate_fails() {
        let calibration = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let candidates = [0.0, 1.0];
        // alpha so small that even lambda = 1.0 (R_hat = 0.0) cannot satisfy
        // the corrected target: 0.01 + (0.01 - 1.0) / 10 = -0.089.
        let alpha = OpenUnitInterval::new("alpha", 0.01).unwrap();
        let err = certify(&ExceedsThreshold, alpha, &calibration, &candidates).unwrap_err();
        assert!(matches!(err, RiskSieveError::NoFeasibleParameter));
    }

    #[test]
    fn uninformative_result_flags_the_conservative_fallback() {
        let calibration = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let candidates = [0.0, 1.0];
        let alpha = OpenUnitInterval::new("alpha", 0.3).unwrap();
        let certificate = certify(&ExceedsThreshold, alpha, &calibration, &candidates).unwrap();
        assert_eq!(certificate.parameter, 1.0);
        assert_eq!(certificate.diagnostics.empirical_risk, Some(0.0));
        assert_eq!(certificate.diagnostics.uninformative_result, Some(true));
    }
}
