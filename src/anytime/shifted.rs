//! Importance-weighted anytime-valid conformal risk control under
//! covariate shift.
//!
//! Hultberg, Zachariah, and Ribeiro (2026), *Anytime-Valid Conformal Risk
//! Control*, arXiv:2602.04364, Theorem 4.7 extends [`super::calibration`]'s
//! Theorem 4.1 to a test distribution `P*` that differs from the
//! calibration distribution `P` by a known importance weight (Radon-Nikodym
//! derivative) `omega(x,y) = dP*(x,y)/dP(x,y)`. The empirical risk stays a
//! plain, unweighted average of the loss over calibration observations
//! (weights never reweight the loss itself); what changes is the
//! correction term:
//!
//! ```text
//! gamma_n = B * (1 - (1/n) * sum_{i=1}^n omega_i) + weighted_term_{m*,delta}(B^2 * W_n) / n
//! W_n     = sum_{i=1}^n omega_i^2
//! ```
//!
//! where `weighted_term` is defined in [`super::boundary`] — see that
//! module's doc for why this is *not* the `h_{B,m,delta}` function every
//! independent fetch of this theorem's text read it as (a dimensional
//! argument, confirmed numerically, overrides the fetched text on this
//! one point). `m*` is redefined analogously to Theorem 4.1's, but its
//! defining condition now depends on the realized weights:
//!
//! ```text
//! m* = min{ m' in N : B*(1 - (1/m') sum_{i=1}^{m'} omega_i) + weighted_term_{m',delta}(B^2 W_{m'}) / m' <= alpha }
//! ```
//!
//! ## `m*` as a runtime-discovered stopping point, not a build-time constant
//!
//! Unlike Theorem 4.1, where `m*` depends only on `(alpha, B, delta)` and
//! is computed once at `build()` time, Theorem 4.7's `m*`-defining
//! condition depends on the actual realized weights up to `m'`, which are
//! not known until that many observations have arrived. This does not
//! make `m*` ill-defined: a `min` found by scanning `m' = 1, 2, 3, ...` in
//! increasing order is fixed permanently the first time the condition
//! holds, because every smaller `m'` has already been checked and ruled
//! out. [`AnytimeShiftedController::update`] therefore checks the
//! condition using the realized weights up to the current step on every
//! call until it first holds, freezes `m*` at that step, and uses the
//! frozen value as the fixed `m` reference for every later step — the
//! same role `m*` plays in Theorem 4.1. Before it freezes, `update`
//! returns the same designated uninformative fallback Theorem 4.1 uses
//! for `n < m*`. That `m*` is a data-dependent stopping time rather than
//! a deterministic constant is a measurability subtlety the paper's proof
//! presumably addresses; this crate did not independently verify that
//! part of the argument (unlike the running-minimum argument below, which
//! it did).
//!
//! ## Known versus estimated weights
//!
//! Theorem 4.7 assumes `omega` is *known* (the paper states this as a
//! standing assumption, not something the theorem itself relaxes) --
//! unlike Bai and Jin (2026), whose Theorem 6.4 gives covariate-shift
//! MDR control a real, if narrow, asymptotic argument for *estimated*
//! weights (see `selective::mdr`'s module docs). Hultberg, Zachariah,
//! and Ribeiro (2026) simply never address the estimated-weight case:
//! there is no analogous robustness result to fall back on here.
//! [`AnytimeShiftedControllerBuilder::weight_source`] requires the caller
//! to say which: [`ImportanceWeightSource::KnownDensityRatio`] yields
//! [`GuaranteeKind::AnytimeHighProbability`], the full theorem-backed
//! claim. [`ImportanceWeightSource::Estimated`] always yields
//! [`GuaranteeKind::EmpiricalOnly`] instead, regardless of what its
//! `consistency` or `threshold_regularity` fields declare -- those exist
//! to support `selective::mdr::certify_weighted`'s own Theorem 6.4
//! hypothesis check, and this controller has no theorem for them to
//! back here (the same "no theorem, no guarantee" reasoning
//! [`crate::nonmonotone::stability`] applies to a fully unsupported
//! `StabilityEvidence`). This crate does not learn `omega` from data
//! itself (AGENTS.md section 3: no automatic density-ratio estimation in
//! the core crate); see [`crate::shift::importance`].
//!
//! ## The running minimum still applies, for the same reason
//!
//! Definition 2.7's guarantee is a joint statement over all `n` on one
//! shared probability event; every `lambda_k` this controller has ever
//! returned already satisfies the bound on that event once it is
//! established (see [`super::calibration`]'s module docs for the full
//! argument, which is about the *structure* of the joint event and does
//! not depend on whether losses are reweighted).
//!
//! **Provenance:** this paper postdates this project's training-data
//! cutoff. Theorem 4.7's additive bias term and its use of `W_n` were
//! read consistently across independent fetches; its boundary-function
//! term was not (see `super::boundary`'s `weighted_term` doc for the
//! correction and the numeric check in `tests/paper_anytime_shifted.rs`
//! that supports it).

use crate::anytime::boundary;
use crate::certificate::{Diagnostics, RiskCertificate};
use crate::error::RiskSieveError;
use crate::guarantee::{
    Assumptions, ExchangeabilityAssumption, GuaranteeKind, ImportanceWeightSource,
    MonotonicityAssumption, ShiftAssumption, StabilityEvidence, SymmetryAssumption,
};
use crate::loss::BoundedLoss;
use crate::probability::{NonNegative, OpenUnitInterval};
use crate::shift::importance::WeightAccumulator;

/// Builder for [`AnytimeShiftedController`].
#[derive(Debug)]
pub struct AnytimeShiftedControllerBuilder<L, Parameter> {
    loss: Option<L>,
    alpha: Option<OpenUnitInterval>,
    delta: Option<OpenUnitInterval>,
    b: Option<f64>,
    candidates: Option<Vec<Parameter>>,
    weight_source: Option<ImportanceWeightSource>,
}

impl<L, Parameter> Default for AnytimeShiftedControllerBuilder<L, Parameter> {
    fn default() -> Self {
        Self {
            loss: None,
            alpha: None,
            delta: None,
            b: None,
            candidates: None,
            weight_source: None,
        }
    }
}

impl<L, Parameter> AnytimeShiftedControllerBuilder<L, Parameter> {
    /// Starts an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the target risk `alpha`.
    pub fn target_risk(mut self, alpha: f64) -> Result<Self, RiskSieveError> {
        self.alpha = Some(OpenUnitInterval::new("alpha", alpha)?);
        Ok(self)
    }

    /// Sets the failure probability `delta`.
    pub fn failure_probability(mut self, delta: f64) -> Result<Self, RiskSieveError> {
        self.delta = Some(OpenUnitInterval::new("delta", delta)?);
        Ok(self)
    }

    /// Sets `B`: the loss must be bounded on `[0, B]`.
    pub fn loss_bound(mut self, b: f64) -> Result<Self, RiskSieveError> {
        if !b.is_finite() {
            return Err(RiskSieveError::NonFiniteValue {
                name: "loss_bound",
                value: b,
            });
        }
        if b <= 0.0 {
            return Err(RiskSieveError::AssumptionMismatch {
                detail: format!("loss_bound must be positive, got {b}"),
            });
        }
        self.b = Some(b);
        Ok(self)
    }

    /// Sets the loss function evaluated on every
    /// [`AnytimeShiftedController::update`].
    pub fn loss(mut self, loss: L) -> Self {
        self.loss = Some(loss);
        self
    }

    /// Sets the ascending candidate grid searched on every update.
    pub fn candidates(mut self, candidates: Vec<Parameter>) -> Self {
        self.candidates = Some(candidates);
        self
    }

    /// Declares whether the importance weights supplied to `update` are a
    /// known density ratio or an estimate — never defaulted, since it
    /// determines whether the resulting certificate can claim
    /// [`GuaranteeKind::AnytimeHighProbability`] or only
    /// [`GuaranteeKind::Asymptotic`].
    pub fn weight_source(mut self, source: ImportanceWeightSource) -> Self {
        self.weight_source = Some(source);
        self
    }

    /// Validates the configuration. Unlike
    /// [`super::calibration::AnytimeControllerBuilder::build`], `m*` is
    /// not computed here: Theorem 4.7's `m*` depends on realized weights
    /// not yet observed (see the module docs).
    pub fn build(self) -> Result<AnytimeShiftedController<L, Parameter>, RiskSieveError>
    where
        Parameter: PartialOrd,
    {
        let alpha = self
            .alpha
            .ok_or_else(|| RiskSieveError::AssumptionMismatch {
                detail: "target_risk is required".to_string(),
            })?;
        let delta = self
            .delta
            .ok_or_else(|| RiskSieveError::AssumptionMismatch {
                detail: "failure_probability is required".to_string(),
            })?;
        let b = self.b.ok_or_else(|| RiskSieveError::AssumptionMismatch {
            detail: "loss_bound is required".to_string(),
        })?;
        let loss = self
            .loss
            .ok_or_else(|| RiskSieveError::AssumptionMismatch {
                detail: "loss is required".to_string(),
            })?;
        let candidates = self
            .candidates
            .ok_or_else(|| RiskSieveError::AssumptionMismatch {
                detail: "candidates is required".to_string(),
            })?;
        let weight_source =
            self.weight_source
                .ok_or_else(|| RiskSieveError::AssumptionMismatch {
                    detail: "weight_source is required".to_string(),
                })?;
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
        if b <= alpha.get() {
            return Err(RiskSieveError::AssumptionMismatch {
                detail: format!(
                    "loss_bound ({b}) must exceed target_risk ({}) for Theorem 4.7's \
                     correction term to stay real-valued",
                    alpha.get()
                ),
            });
        }
        let cumulative_loss = vec![0.0; candidates.len()];

        Ok(AnytimeShiftedController {
            loss,
            alpha,
            delta,
            b,
            candidates,
            weight_source,
            m_star: None,
            n: 0,
            cumulative_loss,
            weights: WeightAccumulator::new(),
            best_index: None,
        })
    }
}

/// Incremental state for importance-weighted anytime-valid conformal
/// risk control under covariate shift (Theorem 4.7).
///
/// Construct via [`AnytimeShiftedController::builder`], then call
/// [`AnytimeShiftedController::update`] once per new calibration
/// observation, supplying its importance weight alongside it.
#[derive(Debug)]
pub struct AnytimeShiftedController<L, Parameter> {
    loss: L,
    alpha: OpenUnitInterval,
    delta: OpenUnitInterval,
    b: f64,
    candidates: Vec<Parameter>,
    weight_source: ImportanceWeightSource,
    m_star: Option<usize>,
    n: usize,
    cumulative_loss: Vec<f64>,
    weights: WeightAccumulator,
    best_index: Option<usize>,
}

impl<L, Parameter> AnytimeShiftedController<L, Parameter> {
    /// Starts a new builder.
    pub fn builder() -> AnytimeShiftedControllerBuilder<L, Parameter> {
        AnytimeShiftedControllerBuilder::new()
    }

    /// The number of calibration observations folded in so far.
    pub fn calibration_size(&self) -> usize {
        self.n
    }

    /// The frozen minimum eligible calibration size `m*`, once
    /// discovered; `None` before Theorem 4.7's `m*`-defining condition
    /// has held for the first time (see the module docs).
    pub fn minimum_eligible_calibration_size(&self) -> Option<usize> {
        self.m_star
    }
}

impl<L, Parameter: Clone + PartialOrd> AnytimeShiftedController<L, Parameter> {
    /// Folds in one new calibration observation and its importance
    /// weight, returning a fresh certificate reflecting all observations
    /// seen so far.
    ///
    /// # Guarantee
    ///
    /// [`GuaranteeKind::AnytimeHighProbability`] when `weight_source` is
    /// [`ImportanceWeightSource::KnownDensityRatio`]:  with probability at
    /// least `1 - delta`, every certificate this controller has ever
    /// returned satisfies `E_{P*}[loss] <= alpha`.
    /// [`GuaranteeKind::Asymptotic`] when weights are
    /// [`ImportanceWeightSource::Estimated`] instead.
    ///
    /// # Errors
    ///
    /// - Any error [`BoundedLoss::evaluate_checked`] returns.
    /// - [`RiskSieveError::AssumptionMismatch`] if `loss.bounds()` does
    ///   not match the configured `[0, B]`.
    /// - [`RiskSieveError::InvalidImportanceWeight`] if `weight` is
    ///   negative, `NaN`, or infinite.
    /// - [`RiskSieveError::DegenerateWeights`] if every weight folded in
    ///   so far (including this one) is exactly zero.
    /// - [`RiskSieveError::NoFeasibleParameter`] if `m*` has been reached
    ///   but no candidate in the grid meets the corrected target.
    ///
    /// # Example
    ///
    /// ```
    /// use risksieve::anytime::AnytimeShiftedController;
    /// use risksieve::{BoundedLoss, ClosedInterval, ImportanceWeightSource, RiskSieveError};
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
    /// let mut controller = AnytimeShiftedController::builder()
    ///     .target_risk(0.9)?
    ///     .failure_probability(0.999)?
    ///     .loss_bound(1.0)?
    ///     .loss(ExceedsThreshold)
    ///     .candidates(vec![0.0, 0.5, 1.0])
    ///     .weight_source(ImportanceWeightSource::KnownDensityRatio)
    ///     .build()?;
    ///
    /// let first = controller.update(&0.05, 1.0)?;
    /// assert_eq!(first.diagnostics.weight_sum, Some(1.0));
    /// # Ok::<(), RiskSieveError>(())
    /// ```
    pub fn update<Observation>(
        &mut self,
        observation: &Observation,
        weight: f64,
    ) -> Result<RiskCertificate<Parameter>, RiskSieveError>
    where
        L: BoundedLoss<Observation, Parameter>,
    {
        let bounds = self.loss.bounds();
        if bounds.lower() != 0.0 || bounds.upper() != self.b {
            return Err(RiskSieveError::AssumptionMismatch {
                detail: format!(
                    "loss.bounds() = [{}, {}] does not match the configured loss_bound [0, {}]",
                    bounds.lower(),
                    bounds.upper(),
                    self.b
                ),
            });
        }
        let validated_weight = NonNegative::new("weight", weight).map_err(|_| {
            RiskSieveError::InvalidImportanceWeight {
                index: self.n,
                value: weight,
            }
        })?;

        self.n += 1;
        self.weights.update(validated_weight);
        self.weights.ensure_not_degenerate()?;

        for (index, candidate) in self.candidates.iter().enumerate() {
            self.cumulative_loss[index] += self.loss.evaluate_checked(observation, candidate)?;
        }

        let weight_sum = self.weights.sum();
        let weight_sum_of_squares = self.weights.sum_of_squares();
        let n = self.n as f64;

        // `m_reference` stands in for the not-yet-frozen `m*` while
        // searching (using the current step as its own tentative
        // reference); once frozen it is the fixed value every later step
        // reuses. The two coincide exactly at the step `m*` freezes, so
        // `gamma_n` is correct in both cases without recomputation.
        let m_reference = self.m_star.unwrap_or(self.n);
        let bias = self.b * (1.0 - weight_sum / n);
        let v = self.b * self.b * weight_sum_of_squares;
        let gamma_n = bias + boundary::weighted_term(m_reference, self.delta.get(), v) / n;

        if self.m_star.is_none() && gamma_n <= self.alpha.get() {
            self.m_star = Some(self.n);
        }

        let corrected_target = self.alpha.get() - gamma_n;
        let last_index = self.candidates.len() - 1;

        let raw_index = if corrected_target < 0.0 {
            // Either m* has not yet been reached (Theorem 4.7's
            // m*-defining condition mirrors this exactly: gamma_n > alpha
            // for every m' < m*), or gamma_n's bias term happened to be
            // large enough this step regardless -- either way the
            // designated uninformative fallback applies.
            last_index
        } else {
            let mut found = None;
            for index in 0..self.candidates.len() {
                if self.cumulative_loss[index] / n <= corrected_target {
                    found = Some(index);
                    break;
                }
            }
            found.ok_or(RiskSieveError::NoFeasibleParameter)?
        };

        // "The threshold sequence will be non-increasing and, when
        // necessary, we use a running minimum" (Theorem 4.1's
        // Introduction; the module docs explain why this still holds
        // here).
        let overridden = match self.best_index {
            Some(best) if raw_index > best => true,
            _ => {
                self.best_index = Some(raw_index);
                false
            }
        };
        let deployed_index = self
            .best_index
            .expect("set immediately above on first update");

        let guarantee = match &self.weight_source {
            ImportanceWeightSource::KnownDensityRatio => GuaranteeKind::AnytimeHighProbability,
            // Unconditional, unlike `selective::mdr::certify_weighted`'s
            // Theorem-6.4-gated downgrade: Theorem 4.7 has no asymptotic
            // argument for estimated weights at all, so no combination of
            // `Estimated`'s fields could ever earn `Asymptotic` here (see
            // the module docs).
            ImportanceWeightSource::Estimated { .. } => GuaranteeKind::EmpiricalOnly,
        };

        let diagnostics = Diagnostics {
            empirical_risk: Some(self.cumulative_loss[deployed_index] / n),
            correction_term: Some(gamma_n),
            effective_sample_size: Some(self.weights.effective_sample_size()),
            weight_range: self.weights.range(),
            weight_sum: Some(weight_sum),
            weight_sum_of_squares: Some(weight_sum_of_squares),
            uninformative_result: Some(deployed_index == last_index),
            running_minimum_applied: Some(overridden),
            ..Default::default()
        };
        let assumptions = Assumptions {
            exchangeability: ExchangeabilityAssumption::CovariateShiftIid,
            bounded_loss: bounds,
            monotonicity: MonotonicityAssumption::Monotone {
                non_increasing: true,
            },
            right_continuity: true,
            symmetry: SymmetryAssumption::NotEstablished,
            stability: StabilityEvidence::Unknown,
            shift: ShiftAssumption::CovariateShift {
                weight_source: self.weight_source.clone(),
            },
        };

        Ok(RiskCertificate {
            parameter: self.candidates[deployed_index].clone(),
            target_risk: self.alpha.get(),
            certified_upper_bound: self.alpha.get(),
            guarantee,
            assumptions,
            calibration_size: self.n,
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guarantee::{ThresholdRegularityEvidence, WeightConsistencyEvidence};
    use crate::probability::ClosedInterval;

    #[derive(Debug)]
    struct ExceedsThreshold;
    impl BoundedLoss<f64, f64> for ExceedsThreshold {
        fn bounds(&self) -> ClosedInterval {
            ClosedInterval::new(0.0, 1.0).unwrap()
        }
        fn evaluate(&self, observation: &f64, parameter: &f64) -> Result<f64, RiskSieveError> {
            Ok(if observation > parameter { 1.0 } else { 0.0 })
        }
    }

    fn small_controller() -> AnytimeShiftedController<ExceedsThreshold, f64> {
        AnytimeShiftedController::builder()
            .target_risk(0.9)
            .unwrap()
            .failure_probability(0.999)
            .unwrap()
            .loss_bound(1.0)
            .unwrap()
            .loss(ExceedsThreshold)
            .candidates(vec![0.0, 0.5, 1.0])
            .weight_source(ImportanceWeightSource::KnownDensityRatio)
            .build()
            .unwrap()
    }

    #[test]
    fn build_rejects_missing_weight_source() {
        let err = AnytimeShiftedController::<ExceedsThreshold, f64>::builder()
            .target_risk(0.9)
            .unwrap()
            .failure_probability(0.1)
            .unwrap()
            .loss_bound(1.0)
            .unwrap()
            .loss(ExceedsThreshold)
            .candidates(vec![0.0, 1.0])
            .build()
            .unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn m_star_is_unset_before_the_condition_first_holds() {
        let controller = small_controller();
        assert_eq!(controller.minimum_eligible_calibration_size(), None);
    }

    #[test]
    fn rejects_invalid_weight() {
        let mut controller = small_controller();
        let err = controller.update(&0.05, -1.0).unwrap_err();
        assert!(matches!(
            err,
            RiskSieveError::InvalidImportanceWeight { index: 0, .. }
        ));
    }

    #[test]
    fn rejects_all_zero_weights() {
        let mut controller = small_controller();
        let err = controller.update(&0.05, 0.0).unwrap_err();
        assert!(matches!(err, RiskSieveError::DegenerateWeights));
    }

    /// Unlike `selective::mdr::certify_weighted`, every `Estimated` case
    /// here downgrades to `EmpiricalOnly` unconditionally -- even one
    /// declaring every field Theorem 6.4 would ask for -- since Theorem
    /// 4.7 has no asymptotic argument for estimated weights at all (see
    /// the module docs).
    #[test]
    fn estimated_weight_source_downgrades_to_empirical_only() {
        let mut controller = AnytimeShiftedController::builder()
            .target_risk(0.9)
            .unwrap()
            .failure_probability(0.999)
            .unwrap()
            .loss_bound(1.0)
            .unwrap()
            .loss(ExceedsThreshold)
            .candidates(vec![0.0, 0.5, 1.0])
            .weight_source(ImportanceWeightSource::Estimated {
                method: "test fixture".to_string(),
                training_data_separate_from_calibration: true,
                consistency: WeightConsistencyEvidence::Asserted {
                    justification: "test fixture".to_string(),
                },
                threshold_regularity: ThresholdRegularityEvidence::Asserted {
                    justification: "test fixture".to_string(),
                },
            })
            .build()
            .unwrap();
        let certificate = controller.update(&0.05, 1.0).unwrap();
        assert_eq!(certificate.guarantee, GuaranteeKind::EmpiricalOnly);
    }

    #[test]
    fn exchangeability_is_covariate_shift_iid_not_plain_iid() {
        let mut controller = small_controller();
        let certificate = controller.update(&0.05, 1.0).unwrap();
        assert_eq!(
            certificate.assumptions.exchangeability,
            ExchangeabilityAssumption::CovariateShiftIid
        );
        assert_ne!(
            certificate.assumptions.exchangeability,
            ExchangeabilityAssumption::Iid
        );
    }

    #[test]
    fn records_weight_diagnostics() {
        let mut controller = small_controller();
        controller.update(&0.05, 1.0).unwrap();
        let certificate = controller.update(&0.05, 2.0).unwrap();
        assert_eq!(certificate.diagnostics.weight_sum, Some(3.0));
        assert_eq!(certificate.diagnostics.weight_sum_of_squares, Some(5.0));
        assert_eq!(certificate.diagnostics.weight_range, Some((1.0, 2.0)));
    }

    #[test]
    fn m_star_freezes_permanently_once_reached() {
        let mut controller = small_controller();
        for i in 0..20 {
            controller.update(&0.05, 1.0).unwrap();
            if let Some(first_m_star) = controller.minimum_eligible_calibration_size() {
                // Feed a very different weight next and confirm m* does
                // not move.
                controller.update(&0.05, 50.0).unwrap();
                assert_eq!(
                    controller.minimum_eligible_calibration_size(),
                    Some(first_m_star),
                    "m* moved after being frozen at step {i}"
                );
                return;
            }
        }
        panic!("m* never froze within 20 constant-weight updates");
    }
}
