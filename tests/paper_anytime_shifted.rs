//! Paper-traceable tests for importance-weighted anytime-valid conformal
//! risk control.
//!
//! Source: Hultberg, Zachariah, and Ribeiro (2026), *Anytime-Valid
//! Conformal Risk Control*, arXiv:2602.04364, Theorem 4.7. See
//! `src/anytime/shifted.rs` and `src/anytime/boundary.rs`'s
//! `weighted_term` doc for the background this file assumes, in
//! particular: every independent fetch of Theorem 4.7's correction term
//! read its boundary-function call as `h_{B,m*,delta}(...)`, which is
//! dimensionally impossible (it would make the shift-corrected bound
//! decay *faster* than the unweighted `1/sqrt(n)` rate it generalizes).
//! `weighted_term` (`f`'s square-root term without the linear `h` term)
//! is used instead. This file's
//! `anytime_theorem_4_7_weighted_correction_never_tightens_the_unweighted_bound`
//! test is the permanent regression guard for that correction: it is the
//! same argument that ruled out the fetched reading, kept in the suite
//! rather than living only in a one-time derivation.

use risksieve::anytime::{AnytimeShiftedController, boundary};
use risksieve::{
    BoundedLoss, ClosedInterval, GuaranteeKind, ImportanceWeightSource, RiskSieveError,
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

fn controller(alpha: f64, delta: f64) -> AnytimeShiftedController<ExceedsThreshold, f64> {
    let candidates: Vec<f64> = (0..=20).map(|i| i as f64 / 20.0).collect();
    AnytimeShiftedController::builder()
        .target_risk(alpha)
        .unwrap()
        .failure_probability(delta)
        .unwrap()
        .loss_bound(1.0)
        .unwrap()
        .loss(ExceedsThreshold)
        .candidates(candidates)
        .weight_source(ImportanceWeightSource::KnownDensityRatio)
        .build()
        .unwrap()
}

/// At constant weight 1, the bias term `B*(1 - mean(omega))` must vanish
/// exactly, `W_n` must equal `n` exactly, and Kish's effective sample
/// size must equal `n` exactly -- the paper's own reduction check
/// (AGENTS.md: "tests for constant weights reducing to the unweighted
/// method").
#[test]
fn anytime_theorem_4_7_constant_weight_summary_statistics_are_exact() {
    let mut c = controller(0.5, 0.1);
    let mut certificate = None;
    for i in 1..=50u32 {
        certificate = Some(c.update(&((i as f64 * 0.37).fract()), 1.0).unwrap());
    }
    let certificate = certificate.unwrap();
    assert_eq!(certificate.diagnostics.weight_sum, Some(50.0));
    assert_eq!(certificate.diagnostics.weight_sum_of_squares, Some(50.0));
    assert_eq!(certificate.diagnostics.effective_sample_size, Some(50.0));
}

/// The central correctness check behind `weighted_term` (see the module
/// doc above): a distribution-shift correction cannot be asymptotically
/// *tighter* than the unweighted Theorem 4.1 bound it generalizes.
///
/// At constant weight 1 with `alpha = 0.5`, `delta = 0.1`, `B = 1`: the
/// unweighted `m* = 26` (independently confirmed via `boundary::m_star`);
/// the weighted `m*` (found by the controller itself, using the
/// self-referential search described in `shifted.rs`) turns out to be
/// `24`, asserted below rather than left as a comment. The expected
/// `weighted > unweighted` relationship was established independently in
/// Python from the formulas in `boundary.rs`'s and `shifted.rs`'s doc
/// comments; the unweighted value here is recomputed live by the crate
/// via `boundary::correction`, not read from a fixture. At `n`
/// well past both `m*` values (`n = 200` here), the weighted correction
/// is `0.241003...` versus the unweighted `0.153418...` -- strictly
/// larger, as it must be. Right at the two `m*` values themselves the
/// comparison is not clean (both corrections sit close to `alpha` by
/// construction there), so this test deliberately checks well past that
/// boundary rather than at it.
#[test]
fn anytime_theorem_4_7_weighted_correction_never_tightens_the_unweighted_bound() {
    let alpha = 0.5;
    let delta = 0.1;
    let b = 1.0;
    let n = 200;

    let mut shifted = controller(alpha, delta);
    let mut last = None;
    for i in 1..=n {
        last = Some(
            shifted
                .update(&((i as f64 * 0.618_033_988_75).fract()), 1.0)
                .unwrap(),
        );
    }
    let weighted_gamma = last.unwrap().diagnostics.correction_term.unwrap();
    assert_eq!(shifted.minimum_eligible_calibration_size(), Some(24));

    let m_star_unweighted = boundary::m_star(alpha, b, delta).unwrap();
    let unweighted_gamma = boundary::correction(alpha, b, delta, m_star_unweighted, n as usize);

    assert!(
        weighted_gamma > unweighted_gamma,
        "weighted correction ({weighted_gamma}) must exceed unweighted ({unweighted_gamma}) \
         at matching n; a shift correction that is *tighter* than the exchangeable bound \
         indicates the boundary function regressed to the dimensionally-impossible `h` reading"
    );
}

/// Estimated weights must not yield the finite-sample
/// `AnytimeHighProbability` guarantee Theorem 4.7 reserves for known
/// density ratios.
#[test]
fn anytime_theorem_4_7_estimated_weights_yield_asymptotic_not_high_probability() {
    let candidates: Vec<f64> = (0..=20).map(|i| i as f64 / 20.0).collect();
    let mut c = AnytimeShiftedController::builder()
        .target_risk(0.5)
        .unwrap()
        .failure_probability(0.1)
        .unwrap()
        .loss_bound(1.0)
        .unwrap()
        .loss(ExceedsThreshold)
        .candidates(candidates)
        .weight_source(ImportanceWeightSource::Estimated {
            method: "logistic density-ratio fit".to_string(),
            training_data_separate_from_calibration: true,
        })
        .build()
        .unwrap();

    let certificate = c.update(&0.5, 1.0).unwrap();
    assert_eq!(certificate.guarantee, GuaranteeKind::Asymptotic);
    assert_ne!(certificate.guarantee, GuaranteeKind::AnytimeHighProbability);
}

// AGENTS.md section 9.3's non-increasing-threshold-sequence invariant,
// exercised here under randomized weights (not just randomized losses),
// which is the new degree of freedom this milestone adds.
proptest::proptest! {
    #[test]
    fn anytime_theorem_4_7_threshold_sequence_is_non_increasing(
        alpha in 0.3f64..0.95,
        delta in 0.05f64..0.5,
        scores in proptest::collection::vec(0.0f64..=1.0, 1..100),
        weights in proptest::collection::vec(0.1f64..5.0, 1..100),
    ) {
        let mut c = controller(alpha, delta);
        let mut previous = f64::INFINITY;
        let n = scores.len().min(weights.len());
        for i in 0..n {
            let certificate = c.update(&scores[i], weights[i]).unwrap();
            proptest::prop_assert!(certificate.parameter <= previous);
            previous = certificate.parameter;
        }
    }
}
