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
/// No variant is checkable from observed data; each is always a
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
    /// Calibration observations are i.i.d. from a distribution `P`, test
    /// observations are i.i.d. from a possibly different distribution
    /// `Q`, and the two obey a declared covariate-shift relationship
    /// (Bai and Jin 2026, Assumption 6.1; Hultberg, Zachariah, and
    /// Ribeiro 2026, Section 4.2's `P*`) -- paired with
    /// `ShiftAssumption::CovariateShift`. This is a genuinely different
    /// claim from `Iid`, not a special case of it: the combined sample
    /// is *not* identically distributed (calibration and test draw from
    /// different laws), only each half is, separately, i.i.d. within
    /// itself.
    CovariateShiftIid,
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

/// Evidence that an estimated importance-weight *sequence* converges to
/// the true density ratio in `L2(P_X)` (Bai and Jin 2026, Theorem 6.4:
/// `||w_bar_n(.) - w(.)||_{L2(P_X)} = o_P(1)`), one of that theorem's
/// four hypotheses for its asymptotic MDR conclusion.
///
/// Not checkable by this crate from a single realized weight estimate --
/// it is a statement about a limiting sequence of estimators, so it is
/// always caller-declared (AGENTS.md section 4), the same category
/// `SymmetryAssumption::CallerAsserted` and `StabilityEvidence::UserSupplied`
/// already use for claims this crate cannot verify from the data it sees.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WeightConsistencyEvidence {
    /// The caller asserts the estimator sequence is `L2(P_X)`-consistent
    /// for the true density ratio.
    Asserted {
        /// The caller's justification, for example a citation for the
        /// estimator's own consistency proof.
        justification: String,
    },
    /// No consistency evidence is claimed.
    Unknown,
}

/// Evidence that Bai and Jin (2026) Theorem 6.4's function
/// `F(t) = E_P[w(X)*l(X)*1{s(X)<=t}] / E_P[w(X)]` is continuous and
/// strictly increasing at `t* = sup{t : F(t) <= gamma}`, the theorem's
/// other hypothesis beyond weight consistency and independent training
/// data.
///
/// Like [`WeightConsistencyEvidence`], this is a population-level
/// regularity condition on an unknown function, not something this crate
/// can check from finitely many observations -- always caller-declared.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ThresholdRegularityEvidence {
    /// The caller asserts `F` is continuous and strictly increasing at
    /// `t*`.
    Asserted {
        /// The caller's justification.
        justification: String,
    },
    /// No regularity evidence is claimed.
    Unknown,
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
    ///
    /// `selective::mdr::certify_weighted` only returns
    /// `GuaranteeKind::Asymptotic` (Bai and Jin 2026, Theorem 6.4) when
    /// *every* one of that theorem's hypotheses is declared here --
    /// `training_data_separate_from_calibration`, `consistency`, and
    /// `threshold_regularity` all hold, *and* the caller passed
    /// `gamma == alpha` exactly -- and downgrades to
    /// `GuaranteeKind::EmpiricalOnly` otherwise, per that function's
    /// module docs. `anytime::AnytimeShiftedController`, by contrast,
    /// downgrades every `Estimated` case to `EmpiricalOnly`
    /// unconditionally, regardless of these fields: Hultberg, Zachariah,
    /// and Ribeiro (2026), Theorem 4.7 never discusses estimated weights
    /// at all (it assumes the importance weight `omega` is known as a
    /// standing hypothesis of the theorem itself), so there is no
    /// asymptotic argument for that controller to condition on -- see
    /// that module's docs.
    Estimated {
        /// Description of the estimation method.
        method: String,
        /// Whether the data used to fit the estimator is disjoint from the
        /// calibration data used to compute the certificate (and, for
        /// Theorem 6.4's single-test-point setting, from the test point
        /// itself).
        training_data_separate_from_calibration: bool,
        /// Evidence that the estimator sequence is `L2(P_X)`-consistent
        /// for the true density ratio (Theorem 6.4's hypothesis).
        consistency: WeightConsistencyEvidence,
        /// Evidence that Theorem 6.4's threshold-regularity condition
        /// holds for the relevant `F` and `t*`.
        threshold_regularity: ThresholdRegularityEvidence,
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

    #[test]
    fn exchangeability_covariate_shift_iid_is_distinct_from_plain_iid() {
        assert_ne!(
            ExchangeabilityAssumption::CovariateShiftIid,
            ExchangeabilityAssumption::Iid
        );
    }

    #[test]
    fn weight_consistency_evidence_variants_are_distinguishable() {
        let asserted = WeightConsistencyEvidence::Asserted {
            justification: "estimator has known L2(P_X) consistency rate".into(),
        };
        assert_ne!(asserted, WeightConsistencyEvidence::Unknown);
    }

    #[test]
    fn threshold_regularity_evidence_variants_are_distinguishable() {
        let asserted = ThresholdRegularityEvidence::Asserted {
            justification: "F is smooth and strictly monotone near t*".into(),
        };
        assert_ne!(asserted, ThresholdRegularityEvidence::Unknown);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn importance_weight_source_estimated_serde_round_trip() {
        let source = ImportanceWeightSource::Estimated {
            method: "logistic density-ratio fit".into(),
            training_data_separate_from_calibration: true,
            consistency: WeightConsistencyEvidence::Asserted {
                justification: "known minimax rate".into(),
            },
            threshold_regularity: ThresholdRegularityEvidence::Unknown,
        };
        let json = serde_json::to_string(&source).unwrap();
        let restored: ImportanceWeightSource = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, source);
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
