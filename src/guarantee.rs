//! The guarantee taxonomy and assumption vocabulary.
//!
//! AGENTS.md section 4 forbids collapsing distinct risk-control guarantees
//! into a single boolean such as `is_valid`. This module gives each
//! guarantee its own [`GuaranteeKind`] variant and requires every
//! certificate to carry a complete [`Assumptions`] record, distinguishing
//! caller-declared assumptions from properties the library actually
//! checked or proved. See `docs/guarantees.md` and `docs/assumptions.md`
//! for the full mapping to the cited papers.

use crate::probability::{ClosedInterval, NonNegative};

/// The precise mathematical quantity a [`crate::certificate::RiskCertificate`]
/// controls.
///
/// Never infer which guarantee applies from context; a certificate always
/// carries its `GuaranteeKind` explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GuaranteeKind {
    /// `E[R(theta_hat)] <= alpha` under the declared assumptions.
    ///
    /// Angelopoulos, Bates, Fisch, Lei, and Schuster (2024), Conformal Risk
    /// Control, arXiv:2208.02814.
    ExpectedRisk,

    /// With probability at least `1 - delta`, risk is `<= alpha` for every
    /// calibration time covered by the certificate.
    ///
    /// Hultberg, Zachariah, and Ribeiro (2026), Anytime-Valid Conformal
    /// Risk Control, arXiv:2602.04364, Definition 2.7.
    AnytimeHighProbability,

    /// `E[L * deploy] <= alpha`.
    ///
    /// Bai and Jin (2026), Conformal Selective Prediction with General Risk
    /// Control, arXiv:2603.24704, SCoRE-MDR.
    MarginalDeploymentRisk,

    /// Expected total deployed risk is `<= alpha * m`.
    TotalDeploymentRisk,

    /// Expected average risk among deployed items is `<= alpha`.
    ///
    /// Bai and Jin (2026), arXiv:2603.24704, SCoRE-SDR.
    SelectiveDeploymentRisk,

    /// The guarantee depends on a limiting argument, such as a consistent
    /// but not exactly known importance-weight estimator. Not a
    /// finite-sample certificate.
    Asymptotic,

    /// No theorem-backed guarantee attaches to this result; it is a
    /// diagnostic only and must not be described as certified.
    EmpiricalOnly,
}

/// Whether calibration and test data are assumed i.i.d. or only
/// exchangeable.
///
/// Neither variant is checkable from observed data; it is always a
/// caller-declared assumption (AGENTS.md section 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExchangeabilityAssumption {
    /// Calibration and test data are drawn i.i.d. from the same
    /// distribution. Implies `Exchangeable`.
    Iid,
    /// Calibration and test data are exchangeable but not necessarily
    /// i.i.d.
    Exchangeable,
}

/// Whether the loss is monotone in the parameter, and if so, in which
/// direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MonotonicityAssumption {
    /// The loss is monotone in the parameter.
    Monotone {
        /// `true` if loss is non-increasing as the parameter increases
        /// (the standard conformal-threshold direction); `false` if
        /// non-decreasing.
        non_increasing: bool,
    },
    /// The loss is not assumed monotone; a non-monotonic method is
    /// required.
    NonMonotone,
}

/// Whether the optimization or selection procedure is invariant to
/// permutations of the calibration data.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SymmetryAssumption {
    /// The procedure is provably permutation-invariant by construction.
    ProvenSymmetric,
    /// The caller asserts symmetry holds; the library did not verify it.
    CallerAsserted {
        /// The caller's justification for the assertion.
        justification: String,
    },
    /// Symmetry does not hold or was not established.
    NotEstablished,
}

/// How an importance weight (density ratio) used to correct for covariate
/// shift was obtained.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ImportanceWeightSource {
    /// The density ratio between test and calibration covariate
    /// distributions is known exactly, not estimated from data.
    KnownDensityRatio,
    /// The density ratio was estimated from data.
    Estimated {
        /// Description of the estimation method.
        method: String,
        /// Whether the data used to fit the estimator is disjoint from the
        /// calibration data used to compute the certificate.
        training_data_separate_from_calibration: bool,
    },
}

/// Whether, and how, the certificate accounts for covariate shift between
/// calibration and test distributions.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ShiftAssumption {
    /// Calibration and test covariates share a distribution; no importance
    /// weighting is applied.
    NoShift,
    /// Calibration and test covariates may differ; the certificate applies
    /// importance weighting per `weight_source`.
    CovariateShift {
        /// Where the importance weights came from.
        weight_source: ImportanceWeightSource,
    },
}

/// A method used to produce an empirical estimate of an algorithmic
/// stability constant.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StabilityEstimationMethod {
    /// Bootstrap resampling of the calibration set.
    Bootstrap {
        /// Number of bootstrap resamples used.
        resamples: usize,
    },
    /// Repeated subsampling without replacement.
    Subsampling {
        /// Size of each subsample.
        subsample_size: usize,
        /// Number of subsamples drawn.
        resamples: usize,
    },
    /// Any other method, described in prose.
    Other {
        /// Description of the method used.
        description: String,
    },
}

/// The strength of evidence behind the beta-stability constant a
/// non-monotonic certificate relies on.
///
/// AGENTS.md section 6.4: a bootstrap estimate must never masquerade as a
/// proven stability constant. Only `Analytic` evidence, combined with the
/// other required assumptions, may back a theorem-backed certificate;
/// `Estimated` evidence yields at most an `Asymptotic` or `EmpiricalOnly`
/// guarantee, and `Unknown` must not produce a non-monotonic risk
/// certificate at all.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StabilityEvidence {
    /// The stability constant follows from a proof tied to the specific
    /// algorithm used, for example a closed-form regularized-ERM bound.
    Analytic {
        /// The proven stability constant.
        beta: NonNegative,
        /// Citation for the proof, for example a paper and theorem number.
        reference: String,
    },
    /// The caller supplies a stability constant on their own authority,
    /// without the library verifying its derivation.
    UserSupplied {
        /// The supplied stability constant.
        beta: NonNegative,
        /// The caller's justification.
        justification: String,
    },
    /// The stability constant was estimated empirically, for example by
    /// bootstrap.
    Estimated {
        /// The point estimate.
        estimate: NonNegative,
        /// The method used to produce the estimate.
        method: StabilityEstimationMethod,
        /// An optional confidence interval around the estimate.
        confidence_interval: Option<(f64, f64)>,
    },
    /// No stability evidence has been supplied.
    Unknown,
}

/// The complete set of assumptions behind a
/// [`crate::certificate::RiskCertificate`].
///
/// Every field is populated regardless of guarantee kind; fields that do
/// not apply to a given method should be set to their most conservative
/// value (for example `ShiftAssumption::NoShift` when no shift correction
/// was requested) rather than omitted.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Assumptions {
    /// Whether calibration and test data are i.i.d. or only exchangeable.
    pub exchangeability: ExchangeabilityAssumption,
    /// The declared bounds of the loss function used.
    pub bounded_loss: ClosedInterval,
    /// Whether, and in which direction, the loss is monotone.
    pub monotonicity: MonotonicityAssumption,
    /// Whether the loss (or the relevant statistic built from it) is
    /// assumed right-continuous, as required by some threshold-search
    /// arguments.
    pub right_continuity: bool,
    /// Whether the optimization procedure is permutation-invariant.
    pub symmetry: SymmetryAssumption,
    /// Evidence behind the algorithmic-stability constant, if the method
    /// relies on one.
    pub stability: StabilityEvidence,
    /// Whether, and how, covariate shift is accounted for.
    pub shift: ShiftAssumption,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stability_evidence_variants_are_distinguishable() {
        let analytic = StabilityEvidence::Analytic {
            beta: NonNegative::new("beta", 0.1).unwrap(),
            reference: "Angelopoulos 2026, Theorem 1".into(),
        };
        let unknown = StabilityEvidence::Unknown;
        assert_ne!(analytic, unknown);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn assumptions_serde_round_trip() {
        let assumptions = Assumptions {
            exchangeability: ExchangeabilityAssumption::Exchangeable,
            bounded_loss: ClosedInterval::new(0.0, 1.0).unwrap(),
            monotonicity: MonotonicityAssumption::Monotone {
                non_increasing: true,
            },
            right_continuity: true,
            symmetry: SymmetryAssumption::ProvenSymmetric,
            stability: StabilityEvidence::Unknown,
            shift: ShiftAssumption::NoShift,
        };
        let json = serde_json::to_string(&assumptions).unwrap();
        let restored: Assumptions = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, assumptions);
    }
}
