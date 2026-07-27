//! Paper-traceable tests for classical monotone conformal risk control.
//!
//! Source: Angelopoulos, Bates, Fisch, Lei, and Schuster (2024), *Conformal
//! Risk Control*, ICLR 2024, arXiv:2208.02814, Theorem 1.
//!
//! Interpretation required for this implementation: the paper states the
//! guarantee for `lambda` ranging over an interval and defines `lambda_hat`
//! as an infimum; this crate instead searches a caller-supplied, ascending,
//! finite candidate grid and returns the smallest candidate meeting the
//! corrected target. This coincides with the paper's infimum whenever the
//! true infimum lies in the candidate grid, which is the case in every
//! fixture below by construction.

use risksieve::crc::monotone::certify;
use risksieve::{BoundedLoss, ClosedInterval, GuaranteeKind, OpenUnitInterval, RiskSieveError};

/// `loss_i(lambda) = 1{ s_i > lambda }`: bounded on `[0, 1]`, non-increasing
/// in `lambda`, and right-continuous (the value at `lambda = s_i` is `0`,
/// matching the limit from the right).
struct ExceedsThreshold;

impl BoundedLoss<f64, f64> for ExceedsThreshold {
    fn bounds(&self) -> ClosedInterval {
        ClosedInterval::new(0.0, 1.0).unwrap()
    }

    fn evaluate(&self, observation: &f64, parameter: &f64) -> Result<f64, RiskSieveError> {
        Ok(if observation > parameter { 1.0 } else { 0.0 })
    }
}

const CALIBRATION: [f64; 10] = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
const CANDIDATES: [f64; 11] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];

/// Hand computation for `alpha = 0.3`, `n = 10`, `B = 1`:
///
/// `corrected_target = alpha + (alpha - B) / n = 0.3 + (0.3 - 1.0) / 10 = 0.23`
///
/// `R_hat(lambda) = |{ s_i > lambda }| / 10` over `CANDIDATES`, in order:
/// `1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.0`.
///
/// The smallest `lambda` with `R_hat(lambda) <= 0.23` is `lambda = 0.8`
/// (`R_hat = 0.2`); `lambda = 0.7` gives `R_hat = 0.3`, which fails.
#[test]
fn crc_theorem_1_bounded_monotone_loss_matches_hand_computation() {
    let alpha = OpenUnitInterval::new("alpha", 0.3).unwrap();
    let certificate = certify(&ExceedsThreshold, alpha, &CALIBRATION, &CANDIDATES).unwrap();

    assert_eq!(certificate.parameter, 0.8);
    assert_eq!(certificate.diagnostics.empirical_risk, Some(0.2));
    assert_eq!(certificate.certified_upper_bound, 0.3);
    assert_eq!(certificate.guarantee, GuaranteeKind::ExpectedRisk);
    assert_eq!(certificate.calibration_size, 10);
    assert_eq!(certificate.diagnostics.uninformative_result, Some(false));
}

/// The correction term reported in diagnostics must match the theorem's
/// closed form `(B - alpha) / n`, not merely produce the right threshold
/// by coincidence.
#[test]
fn crc_theorem_1_correction_term_matches_closed_form() {
    let alpha = OpenUnitInterval::new("alpha", 0.3).unwrap();
    let certificate = certify(&ExceedsThreshold, alpha, &CALIBRATION, &CANDIDATES).unwrap();

    let expected_correction = (1.0 - 0.3) / 10.0;
    let actual = certificate.diagnostics.correction_term.unwrap();
    assert!((actual - expected_correction).abs() < 1e-12);
}

/// When `alpha` is small enough that even the most conservative candidate
/// (`lambda = 1.0`, `R_hat = 0.0`) cannot meet the corrected target
/// (`0.01 + (0.01 - 1.0) / 10 = -0.089`), no candidate is feasible: the
/// certificate is refused rather than silently returning an unguaranteed
/// parameter.
#[test]
fn crc_theorem_1_infeasible_alpha_is_refused_not_silently_returned() {
    let alpha = OpenUnitInterval::new("alpha", 0.01).unwrap();
    let err = certify(&ExceedsThreshold, alpha, &CALIBRATION, &CANDIDATES).unwrap_err();
    assert!(matches!(err, RiskSieveError::NoFeasibleParameter));
}
