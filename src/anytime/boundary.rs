//! Boundary functions for anytime-valid conformal risk control.
//!
//! Implements the correction term from Hultberg, Zachariah, and Ribeiro
//! (2026), *Anytime-Valid Conformal Risk Control*, arXiv:2602.04364,
//! Theorem 4.1. Every function here is a pure, deterministic real-valued
//! function so it can be tested directly against independently computed
//! fixtures, without needing an [`crate::anytime::calibration`] state
//! object.
//!
//! Source formulas (Theorem 4.1):
//!
//! ```text
//! h_{B,m,delta}(v) = 2 log(log2(max(v, m) / m) + 1) + log(pi^2 / (6 delta))
//! f_{B,m,delta}(v) = 1.44 sqrt(v h_{B,m,delta}(v)) + 2.42 B h_{B,m,delta}(v)
//! m*               = min { m' in N : f_{B,m',delta}(alpha(B-alpha)m') / m' <= alpha }
//! gamma_n          = f_{B,m*,delta}(alpha(B-alpha)n) / n
//! ```
//!
//! `log` here is the natural logarithm, matching the paper's own
//! convention for these boundary-crossing-probability constructions.
//!
//! **Provenance:** this paper postdates this project's training-data
//! cutoff, so these formulas were extracted from the paper's own text
//! rather than recalled; see the "Provenance of the formulas below"
//! section of [`crate::anytime::calibration`]'s module documentation for
//! how that was cross-checked and what is not yet independently
//! verified.

use crate::error::RiskSieveError;
use std::f64::consts::PI;

/// `h_{B,m,delta}(v)` from Theorem 4.1. Independent of `B` despite the
/// subscript in the paper's notation: `B` only enters through [`f`].
fn h(m: usize, delta: f64, v: f64) -> f64 {
    let m = m as f64;
    let inner = (v.max(m) / m).log2() + 1.0;
    2.0 * inner.ln() + (PI * PI / (6.0 * delta)).ln()
}

/// `f_{B,m,delta}(v)` from Theorem 4.1.
fn f(b: f64, m: usize, delta: f64, v: f64) -> f64 {
    let hv = h(m, delta, v);
    1.44 * (v * hv).sqrt() + 2.42 * b * hv
}

/// The boundary function Theorem 4.7 (the importance-weighted extension,
/// [`crate::anytime::shifted`]) uses in place of [`f`]: `f`'s square-root
/// term alone, without the `2.42 * B * h(v)` linear term.
///
/// ```text
/// weighted_term_{m,delta}(v) = 1.44 sqrt(v * h_{m,delta}(v))
/// ```
///
/// **Provenance note distinct from the rest of this module:** every
/// independent fetch of Theorem 4.7's correction term read its
/// boundary-function term as `h_{B,m*,delta}(B^2 W_n)` — i.e. `h` called
/// directly, not this function. That reading is dimensionally
/// impossible: `h` alone is a `log(log(v))`-scale quantity, so
/// `h(v)/n` decays like `(log log n)/n`, asymptotically *faster* than
/// the `1/sqrt(n)` rate every other boundary in this crate (and the
/// unweighted Theorem 4.1 correction itself) achieves. A distribution-shift
/// correction cannot be asymptotically *tighter* than the unweighted
/// bound it generalizes. This function (matching the fetches'
/// independently-and-consistently-read `m*`-search formula, which already
/// uses this stripped-down form) is the one that reduces to `f`'s leading
/// term and recovers the same `O(1/sqrt(n))` rate — confirmed numerically
/// against the unweighted [`correction`] at constant weights (see
/// `tests/paper_anytime_shifted.rs`). Treat every fetch of Theorem 4.7's
/// exact functional form as suspect on this one point; the reasoning
/// above, not the fetched text, is why this function has the shape it
/// does.
pub(crate) fn weighted_term(m: usize, delta: f64, v: f64) -> f64 {
    let hv = h(m, delta, v);
    1.44 * (v * hv).sqrt()
}

/// The minimum eligible calibration size `m*` from Theorem 4.1: the
/// smallest `m'` at which the boundary's own defining inequality is
/// satisfiable. For `n < m*`, [`correction`] necessarily exceeds `alpha`
/// (Remark 4.3 in the paper), and callers must fall back to the paper's
/// designated uninformative result rather than search for a threshold.
///
/// Requires `b > alpha` (checked by the caller): the inner term
/// `alpha * (b - alpha)` must be non-negative for the square root in
/// `f_{B,m,delta}` to stay real-valued.
///
/// This is a linear search over `m' = 1, 2, 3, ...`; it is a "keep
/// reference implementations simple and auditable before optimizing"
/// choice (AGENTS.md section 8), not the tightest possible complexity.
/// `m*` only needs to be computed once per controller, not per update.
pub fn m_star(alpha: f64, b: f64, delta: f64) -> Result<usize, RiskSieveError> {
    const SEARCH_CAP: usize = 10_000_000;
    for m in 1..=SEARCH_CAP {
        let v = alpha * (b - alpha) * m as f64;
        if f(b, m, delta, v) / m as f64 <= alpha {
            return Ok(m);
        }
    }
    Err(RiskSieveError::NumericalFailure {
        operation: "anytime::boundary::m_star search exceeded its cap",
    })
}

/// The anytime correction term `gamma_n` from Theorem 4.1, evaluated at
/// calibration size `n` given a precomputed `m_star`.
pub fn correction(alpha: f64, b: f64, delta: f64, m_star: usize, n: usize) -> f64 {
    let v = alpha * (b - alpha) * n as f64;
    f(b, m_star, delta, v) / n as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `weighted_term` is `f`'s square-root term with the positive
    /// `2.42 * B * h(v)` term dropped, so it must never exceed `f` at the
    /// same `(m, delta, v)` -- true by construction, but worth pinning
    /// down as a regression guard given how easy the two are to conflate.
    #[test]
    fn weighted_term_never_exceeds_f_at_the_same_arguments() {
        for &(m, delta, v) in &[(325_usize, 0.10, 100.0), (10, 0.05, 5.0), (1, 0.5, 0.01)] {
            assert!(weighted_term(m, delta, v) <= f(1.0, m, delta, v));
        }
    }

    /// Independently computed in Python from the formulas quoted above
    /// (see the module doc), using the paper's own Section 6.1 simulation
    /// parameters (alpha = 5%, delta = 10%, B = 1):
    ///
    /// ```python
    /// import math
    /// def h(m, delta, v):
    ///     return 2*math.log(math.log2(max(v, m)/m) + 1) + math.log(math.pi**2/(6*delta))
    /// def f(B, m, delta, v):
    ///     hv = h(m, delta, v)
    ///     return 1.44*math.sqrt(v*hv) + 2.42*B*hv
    /// def m_star(alpha, B, delta):
    ///     m = 1
    ///     while True:
    ///         v = alpha*(B-alpha)*m
    ///         if f(B, m, delta, v)/m <= alpha:
    ///             return m
    ///         m += 1
    /// print(m_star(0.05, 1.0, 0.10))  # -> 325
    /// ```
    #[test]
    fn anytime_theorem_4_1_m_star_matches_independent_computation() {
        assert_eq!(m_star(0.05, 1.0, 0.10).unwrap(), 325);
    }

    #[test]
    fn anytime_theorem_4_1_correction_below_m_star_exceeds_alpha() {
        let alpha = 0.05;
        let b = 1.0;
        let delta = 0.10;
        let m = m_star(alpha, b, delta).unwrap();
        for n in [1, 10, m - 1] {
            assert!(correction(alpha, b, delta, m, n) > alpha, "n={n}");
        }
    }

    #[test]
    fn anytime_theorem_4_1_correction_at_m_star_is_at_most_alpha() {
        let alpha = 0.05;
        let b = 1.0;
        let delta = 0.10;
        let m = m_star(alpha, b, delta).unwrap();
        assert!(correction(alpha, b, delta, m, m) <= alpha);
    }

    #[test]
    fn anytime_theorem_4_1_correction_matches_independent_computation() {
        // n = 650 = 2 * m*(0.05, 1.0, 0.10); expected value from the same
        // Python script as above, extended with:
        //   def gamma_n(alpha, B, delta, mstar, n):
        //       v = alpha*(B-alpha)*n
        //       return f(B, mstar, delta, v)/n
        //   print(gamma_n(0.05, 1.0, 0.10, 325, 650))  # -> 0.031024...
        let g = correction(0.05, 1.0, 0.10, 325, 650);
        assert!((g - 0.031_025).abs() < 1e-3);
    }
}
