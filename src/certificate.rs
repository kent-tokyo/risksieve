//! The certificate returned by every controller.
//!
//! AGENTS.md section 6.3: controller methods return a certificate, not
//! only a threshold, so that a caller can answer every question in
//! section 1 (what is controlled, in expectation or high probability,
//! fixed-sample or anytime, under which assumptions) directly from the
//! return value.

use crate::guarantee::{Assumptions, GuaranteeKind};

/// Auxiliary information about how a certificate was produced.
///
/// Every field is optional because not every controller produces every
/// diagnostic. A diagnostic is never part of the theorem-backed guarantee
/// unless the cited theorem uses it (AGENTS.md section 6.3); it exists to
/// help callers understand or debug a result, not to expand what is
/// certified.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Diagnostics {
    /// The empirical risk observed on the calibration data.
    pub empirical_risk: Option<f64>,
    /// The finite-sample or anytime correction term added to the empirical
    /// risk.
    pub correction_term: Option<f64>,
    /// The effective sample size, for example after importance weighting.
    pub effective_sample_size: Option<f64>,
    /// The number of items selected for deployment.
    pub selected_count: Option<usize>,
    /// The fraction of items on which the procedure abstained.
    pub abstention_rate: Option<f64>,
    /// The `(min, max)` range of importance weights used, if any.
    pub weight_range: Option<(f64, f64)>,
    /// The stability constant beta actually used in the computation.
    pub stability_beta: Option<f64>,
    /// Whether a running minimum was applied to enforce a non-increasing
    /// threshold sequence.
    pub running_minimum_applied: Option<bool>,
    /// Whether the returned result is an intentionally uninformative
    /// placeholder, as specified by the underlying paper for degenerate
    /// inputs, rather than a `NoFeasibleParameter` error.
    pub uninformative_result: Option<bool>,
    /// A caller-asserted bound this certificate's hypothesis depended on
    /// but that this crate did not itself verify from data — for example
    /// the reference algorithm's own risk bound required by
    /// `nonmonotone::stability::certify`'s Theorem 1 hypothesis. Recorded
    /// so the certificate remains fully auditable even though the bound
    /// is an input assumption, not a computed statistic (AGENTS.md
    /// section 16: "assumptions are represented in returned metadata").
    pub asserted_reference_bound: Option<f64>,
    /// The risk-adjusted e-value `selective::mdr::certify` computed
    /// (Bai and Jin 2026, Equation 4.1) before thresholding it into a
    /// deployment decision. Recorded so the magnitude behind the
    /// boolean decision stays auditable, not just the decision itself.
    pub risk_adjusted_evalue: Option<f64>,
    /// The calibration-threshold parameter `gamma` (Bai and Jin 2026,
    /// Equation 4.1) actually used to compute `risk_adjusted_evalue`.
    /// Distinct from `target_risk` (`alpha`): recorded because Remark 4.5
    /// warns that `gamma > alpha` remains valid but loses selection power,
    /// so a caller auditing a low-power result needs to see which gamma
    /// produced it.
    pub gamma: Option<f64>,
    /// The eBH critical value `tau_hat` (Bai and Jin 2026, Theorem 3.3)
    /// that determined `selective::sdr::certify`'s selection threshold
    /// `m / (alpha * tau_hat)`. `None` when no candidate `tau` qualified,
    /// meaning the selected set is empty. Recorded for the same reason as
    /// `gamma`: the entire selection hinges on this value, so it must be
    /// recoverable from the certificate rather than discarded once used.
    pub ebh_tau_hat: Option<usize>,
    /// The sum of importance weights folded in so far (AGENTS.md
    /// Milestone 6). Together with `weight_sum_of_squares`, lets a caller
    /// recompute the shift-correction bias term and Kish effective
    /// sample size independently of `effective_sample_size`.
    pub weight_sum: Option<f64>,
    /// The sum of squared importance weights folded in so far (`W_n` in
    /// Hultberg, Zachariah, and Ribeiro 2026, Theorem 4.7).
    pub weight_sum_of_squares: Option<f64>,
}

/// The output of every `risksieve` controller: a parameter together with
/// the exact guarantee it carries, the assumptions that guarantee depends
/// on, and supporting diagnostics.
///
/// `target_risk` and `certified_upper_bound` are the caller's requested
/// alpha and the bound the certificate actually proves, respectively; they
/// are not always equal, since finite-sample corrections can make the
/// certified bound conservative relative to the target.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RiskCertificate<Parameter> {
    /// The certified parameter value, for example a conformal threshold.
    pub parameter: Parameter,
    /// The risk level the caller requested (`alpha`).
    pub target_risk: f64,
    /// The upper bound on risk this certificate actually proves.
    pub certified_upper_bound: f64,
    /// The precise guarantee kind this certificate carries.
    pub guarantee: GuaranteeKind,
    /// The complete assumptions this guarantee depends on.
    pub assumptions: Assumptions,
    /// The number of calibration observations used.
    pub calibration_size: usize,
    /// Supporting diagnostics; never part of the guarantee itself.
    pub diagnostics: Diagnostics,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guarantee::{
        ExchangeabilityAssumption, MonotonicityAssumption, ShiftAssumption, StabilityEvidence,
        SymmetryAssumption,
    };
    use crate::probability::ClosedInterval;

    fn sample_assumptions() -> Assumptions {
        Assumptions {
            exchangeability: ExchangeabilityAssumption::Exchangeable,
            bounded_loss: ClosedInterval::new(0.0, 1.0).unwrap(),
            monotonicity: MonotonicityAssumption::Monotone {
                non_increasing: true,
            },
            right_continuity: true,
            symmetry: SymmetryAssumption::ProvenSymmetric,
            stability: StabilityEvidence::Unknown,
            shift: ShiftAssumption::NoShift,
        }
    }

    #[test]
    fn diagnostics_default_is_empty() {
        let diagnostics = Diagnostics::default();
        assert_eq!(diagnostics.empirical_risk, None);
        assert_eq!(diagnostics.selected_count, None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn certificate_serde_round_trip() {
        let certificate = RiskCertificate {
            parameter: 0.42_f64,
            target_risk: 0.1,
            certified_upper_bound: 0.12,
            guarantee: crate::guarantee::GuaranteeKind::ExpectedRisk,
            assumptions: sample_assumptions(),
            calibration_size: 100,
            diagnostics: Diagnostics {
                empirical_risk: Some(0.09),
                ..Default::default()
            },
        };
        let json = serde_json::to_string(&certificate).unwrap();
        let restored: RiskCertificate<f64> = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, certificate);
    }
}
