//! Bounded loss functions.
//!
//! Every risk-control guarantee in `risksieve` is stated in terms of a loss
//! that is bounded on a declared interval (AGENTS.md section 6.2). A loss
//! implementation declares its bounds via [`BoundedLoss::bounds`]; callers
//! that need the runtime check described in section 6.2 ("every returned
//! loss must be checked against the declared interval") should call
//! [`BoundedLoss::evaluate_checked`] rather than [`BoundedLoss::evaluate`]
//! directly.

use crate::error::RiskSieveError;
use crate::probability::ClosedInterval;

/// A loss function whose output is contractually confined to
/// [`Self::bounds`].
///
/// Implementors provide [`evaluate`](BoundedLoss::evaluate); callers should
/// generally invoke [`evaluate_checked`](BoundedLoss::evaluate_checked),
/// which additionally verifies the contract instead of trusting it.
pub trait BoundedLoss<Observation, Parameter> {
    /// The closed interval every value from [`evaluate`](Self::evaluate) is
    /// contractually confined to.
    fn bounds(&self) -> ClosedInterval;

    /// Computes the loss of `parameter` on `observation`.
    ///
    /// Implementations may return a value outside [`Self::bounds`] only by
    /// way of a bug; such a bug is caught by
    /// [`evaluate_checked`](Self::evaluate_checked), not silently clamped.
    fn evaluate(
        &self,
        observation: &Observation,
        parameter: &Parameter,
    ) -> Result<f64, RiskSieveError>;

    /// Computes the loss and verifies it falls within [`Self::bounds`].
    ///
    /// Returns [`RiskSieveError::LossOutOfBounds`] rather than clamping if
    /// the contract is violated, per AGENTS.md section 6.2.
    fn evaluate_checked(
        &self,
        observation: &Observation,
        parameter: &Parameter,
    ) -> Result<f64, RiskSieveError> {
        let value = self.evaluate(observation, parameter)?;
        let bounds = self.bounds();
        if !bounds.contains(value) {
            return Err(RiskSieveError::LossOutOfBounds {
                value,
                lower: bounds.lower(),
                upper: bounds.upper(),
            });
        }
        Ok(value)
    }
}

/// The 0-1 loss: `0.0` when `parameter` equals `observation`, `1.0`
/// otherwise. Bounded on `[0, 1]`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ZeroOneLoss;

impl<T: PartialEq> BoundedLoss<T, T> for ZeroOneLoss {
    fn bounds(&self) -> ClosedInterval {
        ClosedInterval::new(0.0, 1.0).expect("[0, 1] is always a valid interval")
    }

    fn evaluate(&self, observation: &T, parameter: &T) -> Result<f64, RiskSieveError> {
        Ok(if observation == parameter { 0.0 } else { 1.0 })
    }
}

/// Absolute error `|observation - parameter|`, capped at a fixed `cap` so
/// the loss stays bounded on `[0, cap]`.
///
/// Raw absolute error is unbounded, so a cap is required by construction
/// rather than left implicit; see AGENTS.md section 6.2, "AbsoluteErrorLoss
/// with explicit scaling or cap."
#[derive(Debug, Clone, Copy)]
pub struct AbsoluteErrorLoss {
    cap: f64,
}

impl AbsoluteErrorLoss {
    /// Constructs a loss capped at `cap`. `cap` must be finite and
    /// positive.
    pub fn new(cap: f64) -> Result<Self, RiskSieveError> {
        if cap.is_nan() || cap.is_infinite() {
            return Err(RiskSieveError::NonFiniteValue {
                name: "cap",
                value: cap,
            });
        }
        if cap <= 0.0 {
            return Err(RiskSieveError::AssumptionMismatch {
                detail: format!("AbsoluteErrorLoss cap must be positive, got {cap}"),
            });
        }
        Ok(Self { cap })
    }

    /// Returns the configured cap.
    pub fn cap(&self) -> f64 {
        self.cap
    }
}

impl BoundedLoss<f64, f64> for AbsoluteErrorLoss {
    fn bounds(&self) -> ClosedInterval {
        ClosedInterval::new(0.0, self.cap).expect("cap validated positive at construction")
    }

    fn evaluate(&self, observation: &f64, parameter: &f64) -> Result<f64, RiskSieveError> {
        if observation.is_nan() || observation.is_infinite() {
            return Err(RiskSieveError::NonFiniteValue {
                name: "observation",
                value: *observation,
            });
        }
        if parameter.is_nan() || parameter.is_infinite() {
            return Err(RiskSieveError::NonFiniteValue {
                name: "parameter",
                value: *parameter,
            });
        }
        Ok((observation - parameter).abs().min(self.cap))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_one_loss_matches_and_mismatches() {
        let loss = ZeroOneLoss;
        assert_eq!(loss.evaluate_checked(&1, &1).unwrap(), 0.0);
        assert_eq!(loss.evaluate_checked(&1, &2).unwrap(), 1.0);
    }

    #[test]
    fn absolute_error_loss_caps_and_rejects_bad_cap() {
        let loss = AbsoluteErrorLoss::new(2.0).unwrap();
        assert_eq!(loss.evaluate_checked(&5.0, &0.0).unwrap(), 2.0);
        assert_eq!(loss.evaluate_checked(&0.5, &0.0).unwrap(), 0.5);
        assert!(AbsoluteErrorLoss::new(0.0).is_err());
        assert!(AbsoluteErrorLoss::new(-1.0).is_err());
        assert!(AbsoluteErrorLoss::new(f64::NAN).is_err());
    }

    #[test]
    fn evaluate_checked_catches_out_of_bounds_bug() {
        struct BrokenLoss;
        impl BoundedLoss<(), ()> for BrokenLoss {
            fn bounds(&self) -> ClosedInterval {
                ClosedInterval::new(0.0, 1.0).unwrap()
            }
            fn evaluate(&self, _o: &(), _p: &()) -> Result<f64, RiskSieveError> {
                Ok(5.0)
            }
        }
        let err = BrokenLoss.evaluate_checked(&(), &()).unwrap_err();
        assert!(matches!(err, RiskSieveError::LossOutOfBounds { .. }));
    }
}
