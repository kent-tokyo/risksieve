//! Paper-traceable tests for SCoRE-MDR.
//!
//! Source: Bai and Jin (2026), *Conformal Selective Prediction with
//! General Risk Control*, arXiv:2603.24704, Definition 3.1 (risk-adjusted
//! e-values), Equation 4.1 (their construction), and Algorithm
//! 1/Theorem 3.2 (SCoRE-MDR and its MDR guarantee). See
//! `src/selective/evalue.rs` and `src/selective/mdr.rs` for the module
//! docs this file assumes as background, including why `certified_upper_bound`
//! bounds `E[L * psi_hat]` over the joint draw rather than any single
//! realized decision.
//!
//! The two hand traces below derive Equation 4.1's infimum step by step
//! for the smallest possible calibration set (`n = 1`) so the arithmetic
//! can be checked by hand; `src/selective/evalue.rs`'s unit tests assert
//! the same two fixtures directly against the internal e-value type, and
//! the proptest in `src/selective/mdr.rs` cross-checks Proposition 4.4's
//! closed-form shortcut against this same general computation over many
//! randomized inputs.

use risksieve::selective::evalue::risk_adjusted_evalue;
use risksieve::selective::mdr::certify;
use risksieve::{ClosedUnitInterval, GuaranteeKind, OpenUnitInterval};

fn loss(value: f64) -> ClosedUnitInterval {
    ClosedUnitInterval::new("loss", value).unwrap()
}

/// Hand trace: `n=1`, calibration `(s_1, L_1) = (0.0, 1.0)`, test score
/// `s* = 1.0`, `gamma = 0.5`, so `n+1=2` and `gamma*(n+1) = 1.0`.
///
/// `t_gamma(l) = max{t in {0.0, 1.0} : F(t;l) <= 1.0}`:
/// - `F(0.0;l) = 1.0*1{0<=0} + l*1{1<=0} = 1.0 + 0 = 1.0 <= 1.0`: always
///   feasible.
/// - `F(1.0;l) = 1.0*1{0<=1} + l*1{1<=1} = 1.0 + l`: feasible only at
///   `l = 0` (equality).
///
/// So `t_gamma(0) = 1.0` (the max of two feasible values), giving
/// objective `(n+1)*1{1<=1} / (1.0 + 0) = 2.0`; for every `l > 0`,
/// `t_gamma(l) = 0.0`, and `1{s* <= 0.0} = 1{1.0 <= 0.0}` is false, so the
/// objective is identically `0`. The infimum over `l in [0,1]` is `0`.
///
/// `E_{gamma,n+1} = 0 < 1/alpha` for any `alpha < 1`, so SCoRE-MDR never
/// deploys on this input, regardless of `alpha`.
#[test]
fn score_algorithm_1_mdr_matches_reference() {
    let gamma = OpenUnitInterval::new("gamma", 0.5).unwrap();
    let outcome = risk_adjusted_evalue(&[loss(1.0)], &[0.0], 1.0, gamma).unwrap();
    assert_eq!(outcome.value.get(), 0.0);
    assert!(outcome.feasible_threshold_found);

    let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
    let certificate = certify(&[loss(1.0)], &[0.0], 1.0, alpha, gamma).unwrap();
    assert!(!certificate.parameter, "must abstain: e-value is 0");
    assert_eq!(certificate.guarantee, GuaranteeKind::MarginalDeploymentRisk);
    assert_eq!(certificate.diagnostics.risk_adjusted_evalue, Some(0.0));
}

/// Hand trace: `n=1`, calibration `(s_1, L_1) = (1.0, 0.0)`, test score
/// `s* = 0.0` (the smallest of the two), `gamma = 0.5`, `gamma*(n+1)=1.0`.
///
/// Both `M`-values (`0.0` and `1.0`) give `base_sum = 0` (the single
/// calibration loss is `0`), and the test point's own score is `<=` both,
/// so its indicator is always `1`. At `l=0` the denominator is exactly
/// `0`, so the objective is `(n+1)/0 = +infinity`; at `l=1` the
/// denominator is `1`, giving objective `2.0`. The infimum over `[0,1]`
/// is `2.0`, attained at `l=1`, not approached only in a limit.
///
/// `E_{gamma,n+1} = 2.0 >= 1/alpha` exactly when `alpha >= 0.5`, so this
/// input deploys at `alpha = 0.5` and abstains at any stricter `alpha`.
#[test]
fn score_theorem_3_2_deployment_threshold_matches_reference() {
    let gamma = OpenUnitInterval::new("gamma", 0.5).unwrap();
    let outcome = risk_adjusted_evalue(&[loss(0.0)], &[1.0], 0.0, gamma).unwrap();
    assert_eq!(outcome.value.get(), 2.0);

    let deploys = OpenUnitInterval::new("alpha", 0.5).unwrap();
    let certificate = certify(&[loss(0.0)], &[1.0], 0.0, deploys, gamma).unwrap();
    assert!(certificate.parameter, "must deploy: e-value 2.0 >= 1/0.5");

    let abstains = OpenUnitInterval::new("alpha", 0.4).unwrap();
    let certificate = certify(&[loss(0.0)], &[1.0], 0.0, abstains, gamma).unwrap();
    assert!(!certificate.parameter, "must abstain: e-value 2.0 < 1/0.4");
}

/// A calibration set too small (relative to `gamma`) to support any
/// threshold at all must be distinguishable, in the returned
/// diagnostics, from a genuinely small e-value produced by the test
/// point scoring outside the risk-bearing region.
#[test]
fn score_no_feasible_threshold_is_not_silently_conflated_with_a_real_zero() {
    let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
    let strict_gamma = OpenUnitInterval::new("gamma", 0.1).unwrap();

    // Same fixture as `score_algorithm_1_mdr_matches_reference` (which
    // has a real, feasible zero) but with a `gamma` too strict for even
    // the smallest threshold to be feasible.
    let degenerate = certify(&[loss(1.0)], &[0.0], 0.0, alpha, strict_gamma).unwrap();
    assert_eq!(degenerate.diagnostics.uninformative_result, Some(true));

    let real_zero_gamma = OpenUnitInterval::new("gamma", 0.5).unwrap();
    let real_zero = certify(&[loss(1.0)], &[0.0], 1.0, alpha, real_zero_gamma).unwrap();
    assert_eq!(real_zero.diagnostics.uninformative_result, Some(false));
    assert_eq!(real_zero.diagnostics.risk_adjusted_evalue, Some(0.0));
}
