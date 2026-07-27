//! Validated numeric types for probability-like and bound-like values.
//!
//! `risksieve` never accepts a raw `f64` for a quantity such as `alpha`,
//! `delta`, a probability, or a non-negative weight. Every such value is
//! validated once at construction and carried thereafter as one of the
//! types in this module, per the core API principle in AGENTS.md section
//! 6.1.
//!
//! All three wrapper types additionally reject negative zero: its sign bit
//! would otherwise be a silent, semantically meaningless distinction that
//! leaks into [`Ord`] and (with the `serde` feature) serialized output.

use crate::error::RiskSieveError;
use std::cmp::Ordering;

pub(crate) fn check_finite(name: &'static str, value: f64) -> Result<(), RiskSieveError> {
    if value.is_nan() || value.is_infinite() {
        return Err(RiskSieveError::NonFiniteValue { name, value });
    }
    Ok(())
}

fn is_negative_zero(value: f64) -> bool {
    value == 0.0 && value.is_sign_negative()
}

macro_rules! validated_f64 {
    ($(#[$doc:meta])* $name:ident, $check:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy)]
        pub struct $name(f64);

        impl $name {
            /// Validates `value` and wraps it, or returns the error that
            /// explains why it was rejected. `name` identifies the
            /// parameter in error messages, for example `"alpha"`.
            pub fn new(name: &'static str, value: f64) -> Result<Self, RiskSieveError> {
                check_finite(name, value)?;
                if is_negative_zero(value) {
                    return Err(RiskSieveError::InvalidProbability { name, value });
                }
                let predicate: fn(f64) -> bool = $check;
                if !predicate(value) {
                    return Err(RiskSieveError::InvalidProbability { name, value });
                }
                Ok(Self(value))
            }

            /// Returns the wrapped, validated value.
            pub fn get(self) -> f64 {
                self.0
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl Eq for $name {}

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $name {
            /// Total order via [`f64::total_cmp`]. Sound because
            /// construction already excludes NaN and negative zero.
            fn cmp(&self, other: &Self) -> Ordering {
                self.0.total_cmp(&other.0)
            }
        }
    };
}

validated_f64!(
    /// A value strictly between 0 and 1, exclusive on both ends.
    OpenUnitInterval,
    |v| v > 0.0 && v < 1.0
);

validated_f64!(
    /// A value in `[0, 1]`, inclusive on both ends.
    ClosedUnitInterval,
    |v| (0.0..=1.0).contains(&v)
);

validated_f64!(
    /// A finite value greater than or equal to 0.
    NonNegative,
    |v| v >= 0.0
);

/// A closed interval `[lower, upper]` with `lower <= upper`, used to declare
/// the bounds a [`crate::loss::BoundedLoss`] promises to stay within.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosedInterval {
    lower: f64,
    upper: f64,
}

impl ClosedInterval {
    /// Validates and constructs `[lower, upper]`. Rejects non-finite bounds
    /// and `lower > upper`.
    pub fn new(lower: f64, upper: f64) -> Result<Self, RiskSieveError> {
        check_finite("lower", lower)?;
        check_finite("upper", upper)?;
        if lower > upper {
            return Err(RiskSieveError::AssumptionMismatch {
                detail: format!("lower bound {lower} exceeds upper bound {upper}"),
            });
        }
        Ok(Self { lower, upper })
    }

    /// Returns the lower bound.
    pub fn lower(&self) -> f64 {
        self.lower
    }

    /// Returns the upper bound.
    pub fn upper(&self) -> f64 {
        self.upper
    }

    /// Returns `upper - lower`.
    pub fn width(&self) -> f64 {
        self.upper - self.lower
    }

    /// Returns whether `value` lies within `[lower, upper]`, inclusive.
    pub fn contains(&self, value: f64) -> bool {
        value >= self.lower && value <= self.upper
    }
}

#[cfg(feature = "serde")]
mod serde_support {
    use super::{ClosedInterval, ClosedUnitInterval, NonNegative, OpenUnitInterval};
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

    macro_rules! impl_serde {
        ($name:ident, $label:literal) => {
            impl Serialize for $name {
                fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                    serializer.serialize_f64(self.get())
                }
            }

            impl<'de> Deserialize<'de> for $name {
                fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                    let value = f64::deserialize(deserializer)?;
                    $name::new($label, value).map_err(D::Error::custom)
                }
            }
        };
    }

    impl_serde!(OpenUnitInterval, "deserialized OpenUnitInterval");
    impl_serde!(ClosedUnitInterval, "deserialized ClosedUnitInterval");
    impl_serde!(NonNegative, "deserialized NonNegative");

    #[derive(Serialize, Deserialize)]
    struct ClosedIntervalRepr {
        lower: f64,
        upper: f64,
    }

    impl Serialize for ClosedInterval {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            ClosedIntervalRepr {
                lower: self.lower,
                upper: self.upper,
            }
            .serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for ClosedInterval {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let repr = ClosedIntervalRepr::deserialize(deserializer)?;
            ClosedInterval::new(repr.lower, repr.upper).map_err(D::Error::custom)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_unit_interval_rejects_boundary() {
        assert!(OpenUnitInterval::new("alpha", 0.0).is_err());
        assert!(OpenUnitInterval::new("alpha", 1.0).is_err());
        assert!(OpenUnitInterval::new("alpha", 0.5).is_ok());
    }

    #[test]
    fn closed_unit_interval_accepts_boundary() {
        assert!(ClosedUnitInterval::new("delta", 0.0).is_ok());
        assert!(ClosedUnitInterval::new("delta", 1.0).is_ok());
        assert!(ClosedUnitInterval::new("delta", 1.0 + f64::EPSILON).is_err());
    }

    #[test]
    fn non_negative_rejects_negative() {
        assert!(NonNegative::new("weight", -0.001).is_err());
        assert!(NonNegative::new("weight", 0.0).is_ok());
    }

    #[test]
    fn rejects_nan_and_infinite() {
        assert!(matches!(
            NonNegative::new("weight", f64::NAN),
            Err(RiskSieveError::NonFiniteValue { .. })
        ));
        assert!(matches!(
            NonNegative::new("weight", f64::INFINITY),
            Err(RiskSieveError::NonFiniteValue { .. })
        ));
    }

    #[test]
    fn rejects_negative_zero() {
        assert!(matches!(
            NonNegative::new("weight", -0.0),
            Err(RiskSieveError::InvalidProbability { .. })
        ));
    }

    #[test]
    fn total_order_matches_numeric_order() {
        let low = NonNegative::new("w", 0.1).unwrap();
        let high = NonNegative::new("w", 0.9).unwrap();
        assert!(low < high);
    }

    #[test]
    fn closed_interval_rejects_inverted_bounds() {
        assert!(ClosedInterval::new(1.0, 0.0).is_err());
        assert!(ClosedInterval::new(0.0, 1.0).is_ok());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip_rejects_invalid_payload() {
        let value = OpenUnitInterval::new("alpha", 0.25).unwrap();
        let json = serde_json::to_string(&value).unwrap();
        let restored: OpenUnitInterval = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.get(), value.get());

        let bad: Result<OpenUnitInterval, _> = serde_json::from_str("1.5");
        assert!(bad.is_err());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip_closed_interval() {
        let value = ClosedInterval::new(0.0, 1.0).unwrap();
        let json = serde_json::to_string(&value).unwrap();
        let restored: ClosedInterval = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, value);

        let bad: Result<ClosedInterval, _> = serde_json::from_str(r#"{"lower":1.0,"upper":0.0}"#);
        assert!(bad.is_err());
    }
}
