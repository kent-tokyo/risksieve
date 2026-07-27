//! Incremental empirical-risk state for anytime-valid conformal risk
//! control.
//!
//! [`AnytimeController`] accumulates a monotone bounded loss over a
//! cumulatively growing calibration stream and, on every
//! [`AnytimeController::update`], returns a certificate reflecting all
//! observations seen so far: the anytime correction from Hultberg,
//! Zachariah, and Ribeiro (2026), *Anytime-Valid Conformal Risk Control*,
//! arXiv:2602.04364, Theorem 4.1, combined with a running minimum over
//! every threshold computed so far, so the deployed parameter sequence is
//! non-increasing by construction: "The threshold sequence will be
//! non-increasing and, when necessary, we use a running minimum."
//!
//! This does not claim validity under online model retraining; the cited
//! paper explicitly leaves that as future work.
//!
//! ## Why the running minimum stays valid
//!
//! Definition 2.7 is a *joint* statement over all `n` on one shared
//! probability-`(1 - delta)` event: `P(for all n: R_true(lambda_n) <=
//! alpha) >= 1 - delta`. On that event, every individual `lambda_k` ever
//! produced already satisfies `R_true(lambda_k) <= alpha` as a fixed
//! property of that realized value — it does not depend on when the
//! value is reused. The running minimum `min(lambda_1, ..., lambda_n)`
//! is always exactly equal to one of those already-covered values, so it
//! inherits the same bound on the same event; no separate argument about
//! the direction of monotonicity is needed. The source paper states that
//! a running minimum is used but does not spell out this argument
//! explicitly (Introduction, and Remark 4.3 for the `n < m*` case).
//!
//! ## Provenance of the formulas below
//!
//! Hultberg, Zachariah, and Ribeiro (2026) postdates this project's
//! training-data cutoff, so [`boundary`]'s formulas were extracted from
//! the paper's own HTML rendering (arxiv.org and ar5iv) across four
//! independent fetches that agreed digit-for-digit, rather than recalled
//! from training. The general shape of the boundary function — a
//! `sqrt(v * loglog(v))` term plus a `log(1/delta)`-scaled offset — is a
//! recognized family of time-uniform confidence-sequence boundaries (see
//! Howard, Ramdas, McAuliffe, and Sellke (2021), *Time-Uniform,
//! Nonparametric, Nonasymptotic Confidence Sequences*, which uses a
//! structurally identical `1.7 sqrt(v(loglog(2v) + ...))` construction
//! with different constants), which is independent corroboration that
//! this is a real construction rather than a transcription artifact —
//! but the exact numeric constants (`1.44`, `2.42`) have not been
//! verified against a canonical secondary source and should be
//! re-checked against the published version before relying on this for
//! anything safety-critical.

use crate::anytime::boundary;
use crate::certificate::{Diagnostics, RiskCertificate};
use crate::error::RiskSieveError;
use crate::guarantee::{
    Assumptions, ExchangeabilityAssumption, GuaranteeKind, MonotonicityAssumption, ShiftAssumption,
    StabilityEvidence, SymmetryAssumption,
};
use crate::loss::BoundedLoss;
use crate::probability::OpenUnitInterval;

/// Builder for [`AnytimeController`].
///
/// `loss_bound` is configured explicitly, separately from any
/// [`BoundedLoss`] impl, because [`AnytimeController::update`] is generic
/// over `Observation` per call: there is no single `(Observation,
/// Parameter)` pair to resolve `loss.bounds()` against until the first
/// `update` call, at which point it is cross-checked against this value.
#[derive(Debug)]
pub struct AnytimeControllerBuilder<L, Parameter> {
    loss: Option<L>,
    alpha: Option<OpenUnitInterval>,
    delta: Option<OpenUnitInterval>,
    b: Option<f64>,
    candidates: Option<Vec<Parameter>>,
}

impl<L, Parameter> Default for AnytimeControllerBuilder<L, Parameter> {
    fn default() -> Self {
        Self {
            loss: None,
            alpha: None,
            delta: None,
            b: None,
            candidates: None,
        }
    }
}

impl<L, Parameter> AnytimeControllerBuilder<L, Parameter> {
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
    /// [`AnytimeController::update`].
    pub fn loss(mut self, loss: L) -> Self {
        self.loss = Some(loss);
        self
    }

    /// Sets the ascending candidate grid searched on every update.
    pub fn candidates(mut self, candidates: Vec<Parameter>) -> Self {
        self.candidates = Some(candidates);
        self
    }

    /// Validates the configuration and computes `m*` (Theorem 4.1) once.
    pub fn build(self) -> Result<AnytimeController<L, Parameter>, RiskSieveError>
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
                    "loss_bound ({b}) must exceed target_risk ({}) for Theorem 4.1's \
                     correction term to stay real-valued",
                    alpha.get()
                ),
            });
        }
        let m_star = boundary::m_star(alpha.get(), b, delta.get())?;
        let cumulative_loss = vec![0.0; candidates.len()];

        Ok(AnytimeController {
            loss,
            alpha,
            delta,
            b,
            candidates,
            m_star,
            n: 0,
            cumulative_loss,
            best_index: None,
        })
    }
}

/// Incremental state for anytime-valid monotone conformal risk control.
///
/// Construct via [`AnytimeController::builder`], then call
/// [`AnytimeController::update`] once per new calibration observation.
/// Every observation folded in via `update` remains reflected in every
/// later certificate; none are silently discarded.
#[derive(Debug)]
pub struct AnytimeController<L, Parameter> {
    loss: L,
    alpha: OpenUnitInterval,
    delta: OpenUnitInterval,
    b: f64,
    candidates: Vec<Parameter>,
    m_star: usize,
    n: usize,
    cumulative_loss: Vec<f64>,
    best_index: Option<usize>,
}

impl<L, Parameter> AnytimeController<L, Parameter> {
    /// Starts a new builder.
    pub fn builder() -> AnytimeControllerBuilder<L, Parameter> {
        AnytimeControllerBuilder::new()
    }

    /// The number of calibration observations folded in so far.
    pub fn calibration_size(&self) -> usize {
        self.n
    }

    /// The minimum eligible calibration size `m*` (Theorem 4.1): below
    /// this, [`AnytimeController::update`] always returns the paper's
    /// designated uninformative fallback, since the correction
    /// necessarily exceeds `alpha`.
    pub fn minimum_eligible_calibration_size(&self) -> usize {
        self.m_star
    }
}

impl<L, Parameter: Clone + PartialOrd> AnytimeController<L, Parameter> {
    /// Folds in one new calibration observation and returns a fresh
    /// certificate reflecting all observations seen so far.
    ///
    /// # Guarantee
    ///
    /// [`GuaranteeKind::AnytimeHighProbability`]: with probability at
    /// least `1 - delta`, every certificate this controller has ever
    /// returned (across all calls to `update`) satisfies
    /// `E[loss] <= alpha` (Definition 2.7).
    ///
    /// # Errors
    ///
    /// - Any error [`BoundedLoss::evaluate_checked`] returns.
    /// - [`RiskSieveError::AssumptionMismatch`] if `loss.bounds()` does
    ///   not match the `[0, B]` configured via
    ///   [`AnytimeControllerBuilder::loss_bound`].
    /// - [`RiskSieveError::NoFeasibleParameter`] if the corrected target
    ///   is non-negative but no candidate in the grid meets it; extend
    ///   the candidate grid toward the conservative end if this occurs.
    ///
    /// # Example
    ///
    /// ```
    /// use risksieve::anytime::AnytimeController;
    /// use risksieve::{BoundedLoss, ClosedInterval, RiskSieveError};
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
    /// let mut controller = AnytimeController::builder()
    ///     .target_risk(0.9)?
    ///     .failure_probability(0.999)?
    ///     .loss_bound(1.0)?
    ///     .loss(ExceedsThreshold)
    ///     .candidates(vec![0.0, 0.5, 1.0])
    ///     .build()?;
    ///
    /// // n = 1 < m*, so the first certificate is the designated
    /// // uninformative fallback rather than an error.
    /// let first = controller.update(&0.05)?;
    /// assert_eq!(first.diagnostics.uninformative_result, Some(true));
    ///
    /// // More data lets the controller certify a tighter parameter.
    /// let second = controller.update(&0.05)?;
    /// assert_eq!(second.parameter, 0.5);
    /// # Ok::<(), RiskSieveError>(())
    /// ```
    pub fn update<Observation>(
        &mut self,
        observation: &Observation,
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

        self.n += 1;
        for (index, candidate) in self.candidates.iter().enumerate() {
            self.cumulative_loss[index] += self.loss.evaluate_checked(observation, candidate)?;
        }

        let gamma_n = boundary::correction(
            self.alpha.get(),
            self.b,
            self.delta.get(),
            self.m_star,
            self.n,
        );
        let corrected_target = self.alpha.get() - gamma_n;
        let last_index = self.candidates.len() - 1;
        let n = self.n as f64;

        let raw_index = if corrected_target < 0.0 {
            // Theorem 4.1, Remark 4.3: for n < m*, gamma_n > alpha, so the
            // set { lambda : R_n(lambda) <= alpha - gamma_n } is empty and
            // the designated result is the most conservative candidate.
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
        // necessary, we use a running minimum."
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

        let diagnostics = Diagnostics {
            empirical_risk: Some(self.cumulative_loss[deployed_index] / n),
            correction_term: Some(gamma_n),
            uninformative_result: Some(deployed_index == last_index),
            running_minimum_applied: Some(overridden),
            ..Default::default()
        };
        let assumptions = Assumptions {
            exchangeability: ExchangeabilityAssumption::Iid,
            bounded_loss: bounds,
            monotonicity: MonotonicityAssumption::Monotone {
                non_increasing: true,
            },
            right_continuity: true,
            symmetry: SymmetryAssumption::NotEstablished,
            stability: StabilityEvidence::Unknown,
            shift: ShiftAssumption::NoShift,
        };

        Ok(RiskCertificate {
            parameter: self.candidates[deployed_index].clone(),
            target_risk: self.alpha.get(),
            certified_upper_bound: self.alpha.get(),
            guarantee: GuaranteeKind::AnytimeHighProbability,
            assumptions,
            calibration_size: self.n,
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn small_controller() -> AnytimeController<ExceedsThreshold, f64> {
        AnytimeController::builder()
            .target_risk(0.9)
            .unwrap()
            .failure_probability(0.999)
            .unwrap()
            .loss_bound(1.0)
            .unwrap()
            .loss(ExceedsThreshold)
            .candidates(vec![0.0, 0.5, 1.0])
            .build()
            .unwrap()
    }

    #[test]
    fn build_rejects_missing_fields() {
        let err = AnytimeController::<ExceedsThreshold, f64>::builder()
            .target_risk(0.9)
            .unwrap()
            .build()
            .unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn build_rejects_loss_bound_at_or_below_alpha() {
        let err = AnytimeController::<ExceedsThreshold, f64>::builder()
            .target_risk(0.9)
            .unwrap()
            .failure_probability(0.1)
            .unwrap()
            .loss_bound(0.9)
            .unwrap()
            .loss(ExceedsThreshold)
            .candidates(vec![0.0, 1.0])
            .build()
            .unwrap_err();
        assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
    }

    #[test]
    fn minimum_eligible_calibration_size_matches_theorem_4_1() {
        let controller = small_controller();
        assert_eq!(controller.minimum_eligible_calibration_size(), 2);
    }

    /// Hand/Python-computed fixture (alpha = 0.9, delta = 0.999, B = 1.0,
    /// so m* = 2) that exercises the paper's uninformative fallback
    /// (n = 1), a genuine improvement (n = 2), a running-minimum override
    /// (n = 3), and a step that matches the running minimum exactly
    /// (n = 4). See `boundary.rs` tests for the correction-term
    /// cross-check methodology; this fixture additionally simulated the
    /// full search-and-running-minimum logic in Python to confirm the
    /// expected `raw`/`deployed` index at each step before asserting it
    /// here.
    #[test]
    fn anytime_theorem_4_1_running_minimum_matches_hand_computation() {
        let mut controller = small_controller();

        let c1 = controller.update(&0.05_f64).unwrap();
        assert_eq!(c1.parameter, 1.0);
        assert_eq!(c1.diagnostics.uninformative_result, Some(true));
        assert_eq!(c1.diagnostics.running_minimum_applied, Some(false));
        assert_eq!(c1.diagnostics.empirical_risk, Some(0.0));

        let c2 = controller.update(&0.05_f64).unwrap();
        assert_eq!(c2.parameter, 0.5);
        assert_eq!(c2.diagnostics.uninformative_result, Some(false));
        assert_eq!(c2.diagnostics.running_minimum_applied, Some(false));
        assert_eq!(c2.diagnostics.empirical_risk, Some(0.0));

        // A high score (0.9) would raise the raw threshold back to 1.0,
        // but the running minimum keeps 0.5 deployed.
        let c3 = controller.update(&0.9_f64).unwrap();
        assert_eq!(c3.parameter, 0.5);
        assert_eq!(c3.diagnostics.uninformative_result, Some(false));
        assert_eq!(c3.diagnostics.running_minimum_applied, Some(true));
        assert!((c3.diagnostics.empirical_risk.unwrap() - 1.0 / 3.0).abs() < 1e-12);

        let c4 = controller.update(&0.05_f64).unwrap();
        assert_eq!(c4.parameter, 0.5);
        assert_eq!(c4.diagnostics.running_minimum_applied, Some(false));
        assert_eq!(c4.diagnostics.empirical_risk, Some(0.25));

        assert_eq!(controller.calibration_size(), 4);
    }

    // AGENTS.md section 9.3 names this invariant explicitly: "the
    // anytime threshold sequence follows the monotonicity/running-minimum
    // rule." Unlike the hand-computed fixture above, alpha, delta, and
    // the observation stream are all generated by proptest, so this does
    // not depend on any cooperatively chosen data.
    proptest::proptest! {
        #[test]
        fn anytime_threshold_sequence_is_non_increasing(
            alpha in 0.05f64..0.95,
            delta in 0.05f64..0.95,
            scores in proptest::collection::vec(0.0f64..=1.0, 1..200),
        ) {
            let candidates: Vec<f64> = (0..=20).map(|i| i as f64 / 20.0).collect();
            let mut controller = AnytimeController::builder()
                .target_risk(alpha)
                .unwrap()
                .failure_probability(delta)
                .unwrap()
                .loss_bound(1.0)
                .unwrap()
                .loss(ExceedsThreshold)
                .candidates(candidates)
                .build()
                .unwrap();

            let mut previous = f64::INFINITY;
            for score in scores {
                let certificate = controller.update(&score).unwrap();
                proptest::prop_assert!(certificate.parameter <= previous);
                previous = certificate.parameter;
            }
        }
    }
}
