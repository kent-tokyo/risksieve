//! Structured errors for invalid statistical inputs.
//!
//! `risksieve` never panics on caller-supplied data. Every rejection is a
//! [`RiskSieveError`] variant that names the offending value so callers can
//! report or log it without re-deriving what went wrong.

use thiserror::Error;

/// All ways a `risksieve` API call can reject caller input or fail
/// numerically.
///
/// This list is deliberately extensible: as later milestones add
/// controllers, more specific variants may be appended, but existing
/// variants are not repurposed to mean something new.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum RiskSieveError {
    /// A value required to lie in an open or closed unit interval did not.
    #[error("invalid probability-like value for `{name}`: {value}")]
    InvalidProbability {
        /// Name of the rejected parameter, for example `"alpha"`.
        name: &'static str,
        /// The rejected value.
        value: f64,
    },

    /// A value required to be non-negative was negative.
    #[error("value for `{name}` must be non-negative, got {value}")]
    NegativeValue {
        /// Name of the rejected parameter, for example `"weight"`.
        name: &'static str,
        /// The rejected value.
        value: f64,
    },

    /// A value was NaN or infinite where a finite value was required.
    #[error("non-finite value for `{name}`: {value}")]
    NonFiniteValue {
        /// Name of the rejected parameter.
        name: &'static str,
        /// The rejected value.
        value: f64,
    },

    /// A loss implementation returned a value outside its own declared
    /// bounds.
    #[error("loss {value} is out of declared bounds [{lower}, {upper}]")]
    LossOutOfBounds {
        /// The value the loss function returned.
        value: f64,
        /// The declared lower bound.
        lower: f64,
        /// The declared upper bound.
        upper: f64,
    },

    /// A controller was asked to certify a result from zero calibration
    /// observations.
    #[error("calibration set is empty")]
    EmptyCalibrationSet,

    /// An importance weight was negative, non-finite, or otherwise invalid.
    #[error("invalid importance weight at index {index}: {value}")]
    InvalidImportanceWeight {
        /// Position of the offending weight in the calibration sequence.
        index: usize,
        /// The rejected weight.
        value: f64,
    },

    /// Importance weights were supplied but are numerically unusable, for
    /// example all zero.
    #[error("importance weights are degenerate (all zero or numerically unusable)")]
    DegenerateWeights,

    /// A non-monotonic certificate was requested without stability evidence.
    #[error("stability evidence is required but was not supplied")]
    MissingStabilityEvidence,

    /// The caller's declared assumptions are internally inconsistent or
    /// insufficient for the requested guarantee.
    #[error("assumption mismatch: {detail}")]
    AssumptionMismatch {
        /// Human-readable explanation of the mismatch.
        detail: String,
    },

    /// No parameter value satisfies the requested risk target under the
    /// declared assumptions.
    #[error("no feasible parameter satisfies the requested risk target")]
    NoFeasibleParameter,

    /// An internal numerical routine failed to converge or found its own
    /// invariant violated, for example a bounded search exceeding its cap.
    #[error("numerical failure during `{operation}`")]
    NumericalFailure {
        /// Name of the routine that failed.
        operation: &'static str,
    },

    /// An accumulated quantity (a weight, a sum, a derived correction
    /// term) would have become non-finite from finite inputs. Distinct
    /// from [`RiskSieveError::NumericalFailure`] (a search or invariant
    /// check that failed) -- this specifically means an arithmetic
    /// operation overflowed, so a caller might reasonably respond by
    /// rescaling their inputs rather than treating it as an algorithmic
    /// limitation.
    #[error("numerical overflow during `{operation}`")]
    NumericalOverflow {
        /// Name of the operation that overflowed.
        operation: &'static str,
    },
}
