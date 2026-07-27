//! Paper-traceable tests for non-monotonic conformal risk control.
//!
//! Source: Angelopoulos (2026), *Conformal Risk Control for Non-Monotonic
//! Losses*, arXiv:2602.20151, Theorem 1 (the general symmetry +
//! beta-stability reduction). Only Theorem 1 is implemented so far; see
//! `src/nonmonotone/stability.rs` for what is deliberately deferred and
//! why.
//!
//! Unlike `tests/paper_crc.rs` and `tests/paper_anytime.rs`, there is no
//! independent hand- or Python-computed numeric fixture here: Theorem 1
//! does not itself search for a parameter or compute a correction term,
//! so there is nothing to independently recompute. Instead, this file
//! checks Theorem 1's stated hypothesis and conclusion directly.
//!
//! An earlier version of this file also fed `crc::monotone::certify`'s
//! output back into `certify` and asserted the fields matched, framed as
//! checking Proposition 1 ("monotone CRC is the `beta = 0` special case
//! of Theorem 1"). That assertion was a tautology: `certify` passes
//! `parameter` and `target_risk` straight through regardless of their
//! values, so the check could not have failed no matter what
//! `crc::monotone::certify` returned. It was removed.
//!
//! Proposition 1's actual content is now checked below
//! (`nonmonotone_proposition_1_*`), against a reference algorithm `A*` of
//! this crate's own choosing: the paper's text never named which `A*` it
//! meant precisely enough to transcribe, so rather than guess, these
//! tests use the natural uncorrected oracle `lambda*(D) = inf{lambda :
//! mean_{i=1}^{n+1} L_i(lambda) <= alpha}` (the same threshold search as
//! `crc::monotone::certify`, minus its finite-sample correction term,
//! evaluated on the full oracle dataset including the held-out point).
//! Under that choice, `beta = 0` is an exact per-dataset domination
//! (`certify`'s leave-one-out threshold is never below `lambda*`), not a
//! Monte Carlo estimate needing a tolerance — see the tests' own doc
//! comments for the algebra.
//!
//! That domination is checked against two different losses on purpose.
//! With the 0/1 indicator loss `ExceedsThreshold`, the risk-level
//! statement (not just the threshold-level one) always collapses to an
//! *exact equality* at the held-out point: whenever `L_i(lambda*(D)) =
//! B` (the held-out point's own loss is already at the bound), `lambda*`
//! turns out to be feasible for `certify(D_{-i})` too — its slack term
//! `B/(n+1)` exactly cancels the held-out point's own worst-case
//! contribution — which forces `lambda_hat(D_{-i}) = lambda*(D)` exactly
//! (combined with the domination proven above); and whenever `L_i(lambda*(D))
//! < B`, the held-out observation sits at or below `lambda*(D)`, which is
//! at or below any larger `lambda_hat(D_{-i})`, so a non-increasing 0/1
//! loss is `0` at both. Either way the two risk values coincide — a
//! genuine structural fact of this loss and this self-referential
//! construction, not a coincidence of the fixture (no choice of sample
//! size or candidate grid changes it; verified by scanning several
//! sample sizes before writing this down). `RampLoss` below is a
//! continuously-varying loss, chosen so `L_i(lambda*(D)) < B` can
//! actually happen, which is what lets the risk-level inequality be
//! strict.

use risksieve::crc::monotone;
use risksieve::nonmonotone::stability::certify;
use risksieve::{
    BoundedLoss, ClosedInterval, NonNegative, OpenUnitInterval, RiskSieveError, StabilityEvidence,
    SymmetryAssumption,
};

struct ExceedsThreshold;
impl BoundedLoss<f64, f64> for ExceedsThreshold {
    fn bounds(&self) -> ClosedInterval {
        ClosedInterval::new(0.0, 1.0).unwrap()
    }
    fn evaluate(&self, observation: &f64, parameter: &f64) -> Result<f64, RiskSieveError> {
        Ok(if observation > parameter { 1.0 } else { 0.0 })
    }
}

/// A continuously-varying bounded monotone loss (unlike `ExceedsThreshold`'s
/// 0/1 step), so that a one-grid-step move in the certified parameter can
/// produce a genuinely intermediate, non-`{0, B}` loss value at the
/// held-out point.
struct RampLoss;
impl BoundedLoss<f64, f64> for RampLoss {
    fn bounds(&self) -> ClosedInterval {
        ClosedInterval::new(0.0, 1.0).unwrap()
    }
    fn evaluate(&self, observation: &f64, parameter: &f64) -> Result<f64, RiskSieveError> {
        Ok((observation - parameter).clamp(0.0, 1.0))
    }
}

/// `lambda*(D) = inf{lambda : mean_{i=1}^{n+1} L_i(lambda) <= alpha}` —
/// `certify`'s own threshold search with the `+ (alpha - B)/n` correction
/// dropped, scanned over the same candidate grid so the comparison in
/// the tests below is apples-to-apples.
fn oracle_lambda_star<L: BoundedLoss<f64, f64>>(
    loss: &L,
    alpha: f64,
    observations: &[f64],
    candidates: &[f64],
) -> f64 {
    for &candidate in candidates {
        let mean = observations
            .iter()
            .map(|observation| loss.evaluate(observation, &candidate).unwrap())
            .sum::<f64>()
            / observations.len() as f64;
        if mean <= alpha {
            return candidate;
        }
    }
    panic!("oracle infeasible: extend the candidate grid toward 1.0");
}

/// Runs the leave-one-out-vs-oracle comparison for every held-out point
/// in `observations`, asserting the domination proven in
/// `nonmonotone_proposition_1_leave_one_out_domination_is_non_vacuous`'s
/// doc comment on every single point (not just in aggregate), and
/// returning summary counts/averages for the tests to pin.
struct LeaveOneOutTrace {
    lambda_star: f64,
    strict_threshold_count: usize,
    strict_loss_count: usize,
    loo_average: f64,
    oracle_average: f64,
}

fn leave_one_out_trace<L: BoundedLoss<f64, f64>>(
    loss: &L,
    alpha: f64,
    observations: &[f64],
    candidates: &[f64],
) -> LeaveOneOutTrace {
    let alpha_typed = OpenUnitInterval::new("alpha", alpha).unwrap();
    let lambda_star = oracle_lambda_star(loss, alpha, observations, candidates);
    let n_total = observations.len();

    let mut strict_threshold_count = 0usize;
    let mut strict_loss_count = 0usize;
    let mut loo_losses = Vec::with_capacity(n_total);
    let mut oracle_losses = Vec::with_capacity(n_total);

    for i in 0..n_total {
        let mut rest = observations.to_vec();
        let held_out = rest.remove(i);

        let certificate = monotone::certify(loss, alpha_typed, &rest, candidates).unwrap();
        let lambda_hat = certificate.parameter;
        assert!(
            lambda_hat >= lambda_star,
            "held-out index {i}: lambda_hat ({lambda_hat}) fell below the oracle lambda* ({lambda_star})"
        );
        if lambda_hat > lambda_star {
            strict_threshold_count += 1;
        }

        let loo_loss = loss.evaluate(&held_out, &lambda_hat).unwrap();
        let oracle_loss = loss.evaluate(&held_out, &lambda_star).unwrap();
        assert!(
            loo_loss <= oracle_loss,
            "held-out index {i}: leave-one-out loss ({loo_loss}) exceeded the oracle loss ({oracle_loss})"
        );
        if loo_loss != oracle_loss {
            strict_loss_count += 1;
        }
        loo_losses.push(loo_loss);
        oracle_losses.push(oracle_loss);
    }

    LeaveOneOutTrace {
        lambda_star,
        strict_threshold_count,
        strict_loss_count,
        loo_average: loo_losses.iter().sum::<f64>() / n_total as f64,
        oracle_average: oracle_losses.iter().sum::<f64>() / n_total as f64,
    }
}

fn analytic(beta: f64) -> StabilityEvidence {
    StabilityEvidence::Analytic {
        beta: NonNegative::new("beta", beta).unwrap(),
        reference: "Angelopoulos (2026), Theorem 1".to_string(),
    }
}

/// Theorem 1's exact hypothesis: `reference_risk_bound <= alpha - beta`.
/// A reference bound exactly at the boundary is accepted; one epsilon
/// past it is refused. This is the hypothesis stated in the paper, not
/// an implementation choice, so it is checked precisely rather than with
/// a loose inequality.
#[test]
fn nonmonotone_theorem_1_hypothesis_boundary_is_exact() {
    let alpha = OpenUnitInterval::new("alpha", 0.2).unwrap();
    let beta = 0.05;
    let boundary = alpha.get() - beta;

    let ok = certify(
        (),
        alpha,
        boundary,
        ClosedInterval::new(0.0, 1.0).unwrap(),
        analytic(beta),
        SymmetryAssumption::ProvenSymmetric,
        1,
    );
    assert!(ok.is_ok());

    let just_over = boundary + 1e-9;
    let err = certify(
        (),
        alpha,
        just_over,
        ClosedInterval::new(0.0, 1.0).unwrap(),
        analytic(beta),
        SymmetryAssumption::ProvenSymmetric,
        1,
    )
    .unwrap_err();
    assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
}

/// A larger beta strictly tightens the feasible reference-bound region,
/// matching Theorem 1's `alpha - beta` term directly.
#[test]
fn nonmonotone_theorem_1_larger_beta_requires_tighter_reference_bound() {
    let alpha = OpenUnitInterval::new("alpha", 0.3).unwrap();
    let reference_bound = 0.2;

    // beta = 0.05: alpha - beta = 0.25 >= 0.2, feasible.
    assert!(
        certify(
            (),
            alpha,
            reference_bound,
            ClosedInterval::new(0.0, 1.0).unwrap(),
            analytic(0.05),
            SymmetryAssumption::ProvenSymmetric,
            1,
        )
        .is_ok()
    );

    // beta = 0.2: alpha - beta = 0.1 < 0.2, infeasible.
    let err = certify(
        (),
        alpha,
        reference_bound,
        ClosedInterval::new(0.0, 1.0).unwrap(),
        analytic(0.2),
        SymmetryAssumption::ProvenSymmetric,
        1,
    )
    .unwrap_err();
    assert!(matches!(err, RiskSieveError::AssumptionMismatch { .. }));
}

/// Proposition 1's `beta = 0` claim, against the oracle `A*` this file's
/// module doc defines: for every held-out point `i`, `certify` on the
/// other `n` points picks a threshold at least as large as `lambda*`,
/// because `certify`'s feasibility condition is provably tighter.
///
/// For any candidate `lambda` and the `n` points excluding `i`, `certify`'s
/// own feasibility quantity `(n/(n+1)) R_hat_{-i}(lambda) + B/(n+1)`
/// equals `(1/(n+1)) [sum_{j != i} L_j(lambda) + B]`, which is at least
/// `(1/(n+1)) [sum_{j != i} L_j(lambda) + L_i(lambda)]` since `L_i <= B`,
/// which is exactly `mean_{j=1}^{n+1} L_j(lambda)`, the oracle's own
/// feasibility quantity. So `certify`'s feasible set (built from the
/// larger quantity) is a subset of the oracle's feasible set (built from
/// the smaller one), and since both are non-decreasing-loss
/// upward-closed rays, the smaller set's infimum is at least the larger
/// set's: `lambda_hat(D_{-i})` is at least `lambda*(D)`. Loss is
/// non-increasing in the parameter, so this threshold ordering flows
/// through to `L_i(lambda_hat(D_{-i}))` being at most `L_i(lambda*(D))`,
/// and then to the two quantities' averages.
///
/// A candidate grid coarse enough that every held-out fit coincides with
/// the oracle would make the threshold-level part of this pass
/// vacuously (both sides bitwise equal for a reason that has nothing to
/// do with Proposition 1), so this test also pins a non-trivial fraction
/// of held-out points that actually landed strictly above the oracle.
/// The risk-level part is *not* pinned to a nonzero strict count here —
/// per the module doc, `ExceedsThreshold`'s 0/1 structure makes the
/// risk-level statement collapse to an exact equality on every point,
/// which is itself the fact this test pins;
/// `nonmonotone_proposition_1_leave_one_out_can_strictly_reduce_risk`
/// below is where the risk-level inequality is exercised as strict.
#[test]
fn nonmonotone_proposition_1_leave_one_out_domination_is_non_vacuous() {
    let alpha = 0.5;
    let candidates: Vec<f64> = (0..=100).map(|i| i as f64 / 100.0).collect();
    let observations: Vec<f64> = (1..=40)
        .map(|i| (i as f64 * 0.618_033_988_75).fract())
        .collect();

    let trace = leave_one_out_trace(&ExceedsThreshold, alpha, &observations, &candidates);

    // Pinned to the exact value for this fixed observation sequence
    // (independently recomputed in Python), not just "some": exactly 20
    // of the 40 held-out fits land strictly above the oracle, so the
    // threshold-level domination isn't a coincidence of a too-coarse
    // candidate grid.
    assert_eq!(
        trace.strict_threshold_count, 20,
        "expected exactly 20/40 held-out fits strictly above the oracle, got {} \
         -- candidate grid or observation sequence may have changed",
        trace.strict_threshold_count
    );
    // The structural collapse explained in the module doc: with this
    // loss, the risk-level statement is never once strict.
    assert_eq!(trace.strict_loss_count, 0);
    assert_eq!(trace.loo_average, 0.5);
    assert_eq!(trace.oracle_average, 0.5);
}

/// The risk-level half of Proposition 1's claim, exercised as a strict
/// inequality: `RampLoss` (module doc) is continuously-varying, so
/// `L_i(lambda*(D)) < B` can actually happen, which is exactly the
/// condition under which the structural collapse in
/// `nonmonotone_proposition_1_leave_one_out_domination_is_non_vacuous`'s
/// doc comment does not apply and the domination proven there can be
/// strict at the risk level too.
#[test]
fn nonmonotone_proposition_1_leave_one_out_can_strictly_reduce_risk() {
    let alpha = 0.5;
    let candidates: Vec<f64> = (0..=100).map(|i| i as f64 / 100.0).collect();
    let observations: Vec<f64> = (1..=40)
        .map(|i| (i as f64 * 0.618_033_988_75).fract())
        .collect();

    let trace = leave_one_out_trace(&RampLoss, alpha, &observations, &candidates);

    // Pinned to the exact value for this fixed observation sequence
    // (independently recomputed in Python): 32 of the 40 held-out points
    // see a strictly lower leave-one-out loss than the oracle's, so the
    // risk-level inequality is genuinely exercised here, not vacuous.
    assert_eq!(
        trace.strict_loss_count, 32,
        "expected exactly 32/40 held-out points with a strictly reduced leave-one-out loss, got {} \
         -- candidate grid or observation sequence may have changed",
        trace.strict_loss_count
    );
    assert!(
        trace.oracle_average - trace.loo_average > 0.005,
        "leave-one-out average risk ({}) should be non-trivially below the oracle average ({})",
        trace.loo_average,
        trace.oracle_average
    );
    let _ = trace.lambda_star; // documented for completeness; not asserted on directly here.
}

/// Shared body for the fuzzed domination check below, generic over the
/// loss so it runs against both `ExceedsThreshold` and `RampLoss` without
/// duplicating the loop (the domination itself holds for any bounded
/// monotone loss, not just these two).
fn check_domination_property<L: BoundedLoss<f64, f64>>(
    loss: &L,
    alpha: f64,
    observations: &[f64],
    candidates: &[f64],
) -> proptest::test_runner::TestCaseResult {
    let alpha_typed = OpenUnitInterval::new("alpha", alpha).unwrap();
    let lambda_star = oracle_lambda_star(loss, alpha, observations, candidates);
    let n_total = observations.len();

    for i in 0..n_total {
        let mut rest = observations.to_vec();
        let held_out = rest.remove(i);

        let certificate = monotone::certify(loss, alpha_typed, &rest, candidates).unwrap();
        let lambda_hat = certificate.parameter;
        proptest::prop_assert!(lambda_hat >= lambda_star);

        let loo_loss = loss.evaluate(&held_out, &lambda_hat).unwrap();
        let oracle_loss = loss.evaluate(&held_out, &lambda_star).unwrap();
        proptest::prop_assert!(loo_loss <= oracle_loss);
    }
    Ok(())
}

proptest::proptest! {
    /// The same domination argument as
    /// `nonmonotone_proposition_1_leave_one_out_domination_is_non_vacuous`,
    /// fuzzed over random datasets rather than one fixed sequence, and
    /// checked against both losses defined above.
    #[test]
    fn nonmonotone_proposition_1_leave_one_out_never_beats_the_oracle(
        observations in proptest::collection::vec(0.0f64..=1.0, 3..25),
    ) {
        let alpha = 0.5;
        let candidates: Vec<f64> = (0..=40).map(|i| i as f64 / 40.0).collect();

        check_domination_property(&ExceedsThreshold, alpha, &observations, &candidates)?;
        check_domination_property(&RampLoss, alpha, &observations, &candidates)?;
    }
}
